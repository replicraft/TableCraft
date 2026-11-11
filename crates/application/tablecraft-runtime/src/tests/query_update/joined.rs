//! Real joined review stages source values before the existing native writer/save.
use super::{assert_image, inspect, open, opened, request, save_and_acknowledge};
use crate::{Command, Error, Response};
use tablecraft_access::editing::sql::RecoverableError;
use tablecraft_query::{QueryError, UpdateError};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes, joined_update,
};

#[test]
fn joined_update_review_save_ack_and_reopen_preserve_the_independent_native_image() {
    let source = joined_update::source();
    let before = source.bytes;
    let definition = source.target_definition;
    let expected = field_changes::update_score(&before, definition, 0, Some(25));
    let (_directory, path, worker, revision) = opened(&before);
    let sql = "UPDATE Table1 AS t INNER JOIN Changes AS c ON t.ID=c.Id SET t.Score=c.Score+20 WHERE t.ID=1";
    let (target, _) = inspect(&worker, revision, sql);
    let (stale, _) = inspect(&worker, revision, sql);
    let Response::QueryUpdateReviewed { review, capability } = request(
        &worker,
        Command::ReviewQueryUpdate {
            target,
            parameters: Vec::new(),
        },
    )
    .unwrap() else {
        panic!("expected a real source-bound joined UPDATE review");
    };
    assert_eq!(review.table, "Table1");
    assert_eq!(review.document_revision, revision);
    assert_eq!((review.matched_rows, review.changed_rows), (1, 1));
    assert_image(&std::fs::read(&path).unwrap(), &before);
    assert!(!crate::journal_path(&path).exists());
    let receipt = save_and_acknowledge(capability, crate::journal_path(&path));
    assert_eq!(
        (receipt.matched_rows, receipt.changed_rows),
        (Some(1), Some(1))
    );
    assert!(receipt.schema_change.is_none());
    assert_image(&std::fs::read(&path).unwrap(), &expected);
    request(&worker, open(&path, "")).unwrap();
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryUpdate {
                target: stale,
                parameters: Vec::new()
            }
        ),
        Err(Error::StaleQuery)
    ));
    let Response::Data { columns, rows, .. } = request(
        &worker,
        Command::Page {
            table: "Table1".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected fresh target rows");
    };
    assert_eq!(columns[1].native_number, Some(1));
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0][0].text, "1");
    assert_eq!(rows[0][1].text, "25");
    assert_eq!(rows[1][1].text, "NULL");
    let Response::Data { rows, .. } = request(
        &worker,
        Command::Page {
            table: "Changes".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected unchanged joined source rows");
    };
    assert_eq!(rows[0][1].text, "5");
    assert_eq!(rows[1][1].text, "7");
    assert_image(&std::fs::read(&path).unwrap(), &expected);
    assert!(!crate::journal_path(&path).exists());
}

#[test]
fn joined_late_rhs_failure_and_repeated_target_matches_create_no_save_or_journal() {
    let before = joined_update::source().bytes;
    let (_directory, path, worker, revision) = opened(&before);
    for (sql, expected) in [
        (
            "UPDATE Table1 AS t INNER JOIN Changes AS c ON t.ID=c.Id SET t.Score=10 \\ (c.Score-7)",
            QueryError::Evaluation(tablecraft_expressions::EvalError::DivisionByZero),
        ),
        (
            "UPDATE Table1 AS t INNER JOIN Changes AS c ON t.ID=1 SET t.Score=c.Score",
            QueryError::Unsupported,
        ),
    ] {
        let (target, _) = inspect(&worker, revision, sql);
        assert!(
            matches!(request(&worker, Command::ReviewQueryUpdate { target, parameters: Vec::new() }), Err(Error::QueryUpdate(RecoverableError::Plan(UpdateError::Query(actual)))) if actual == expected)
        );
        assert_image(&std::fs::read(&path).unwrap(), &before);
        assert!(!crate::journal_path(&path).exists());
    }
}
