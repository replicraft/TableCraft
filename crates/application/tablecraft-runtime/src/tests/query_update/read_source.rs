//! Real predicate reads retain one immutable native target and consuming save.
use super::{assert_image, inspect, open, opened, request, save_and_acknowledge};
use crate::{Command, Error, Response};
use tablecraft_access::editing::sql::RecoverableError;
use tablecraft_query::{QueryError, UpdateError};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes, joined_update,
};

#[test]
fn correlated_predicates_save_exact_original_handle_updates_and_checked_empty_results() {
    for (sql, matches) in [
        (
            "UPDATE Table1 AS t SET t.Score=25 WHERE EXISTS (SELECT c.Id FROM Changes AS c WHERE c.Id=t.ID AND c.Score=5)",
            1,
        ),
        (
            "UPDATE Table1 SET Score=25 WHERE ID IN (SELECT Id FROM Changes WHERE Score=5)",
            1,
        ),
        (
            "UPDATE Table1 SET Score=25 WHERE ID NOT IN (SELECT NULL FROM Changes)",
            0,
        ),
    ] {
        let source = joined_update::source();
        let expected = if matches == 0 {
            source.bytes.to_vec()
        } else {
            field_changes::update_score(&source.bytes, source.target_definition, 0, Some(25))
        };
        let (_directory, path, worker, revision) = opened(&source.bytes);
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
            panic!("expected explicit predicate-read UPDATE review");
        };
        assert_eq!(
            (review.matched_rows, review.changed_rows),
            (matches, matches)
        );
        assert_image(&std::fs::read(&path).unwrap(), &source.bytes);
        assert!(!crate::journal_path(&path).exists());
        let receipt = save_and_acknowledge(capability, crate::journal_path(&path));
        assert_eq!(
            (receipt.matched_rows, receipt.changed_rows),
            (Some(matches), Some(matches))
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
        let Response::Data { rows, columns, .. } = request(
            &worker,
            Command::Page {
                table: "Table1".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("fresh target rows");
        };
        assert_eq!(columns[1].native_number, Some(1));
        assert_eq!(rows[0][1].text, if matches == 0 { "17" } else { "25" });
        assert_eq!(rows[1][1].text, "NULL");
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Page {
                table: "Changes".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("preserved read source");
        };
        assert_eq!(rows[0][1].text, "5");
        assert_eq!(rows[1][1].text, "7");
        assert_image(&std::fs::read(&path).unwrap(), &expected);
        assert!(!crate::journal_path(&path).exists());
    }
}

#[test]
fn late_correlated_error_and_scalar_assignment_subquery_publish_no_update() {
    let source = joined_update::source();
    let (_directory, path, worker, revision) = opened(&source.bytes);
    for (sql, expected) in [
        (
            "UPDATE Table1 AS t SET t.Score=25 WHERE EXISTS (SELECT 10 \\ (c.Score-7) FROM Changes AS c WHERE c.Id=t.ID OR t.ID=1)",
            QueryError::Evaluation(tablecraft_expressions::EvalError::DivisionByZero),
        ),
        (
            "UPDATE Table1 SET Score=(SELECT Score FROM Changes WHERE Id=1)",
            QueryError::Unsupported,
        ),
    ] {
        let (target, _) = inspect(&worker, revision, sql);
        assert!(
            matches!(request(&worker, Command::ReviewQueryUpdate { target, parameters: Vec::new() }),
            Err(Error::QueryUpdate(RecoverableError::Plan(UpdateError::Query(actual)))) if actual == expected)
        );
        assert_image(&std::fs::read(&path).unwrap(), &source.bytes);
        assert!(!crate::journal_path(&path).exists());
    }
}
