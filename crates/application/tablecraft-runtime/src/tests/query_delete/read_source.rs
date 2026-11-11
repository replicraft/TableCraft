//! Actual source reads finish before existing native DELETE review/publication.
use super::{inspect, open, opened, request, terminal};
use crate::{Command, Error, Response, SaveOutcome, SaveWorker};
use tablecraft_access::editing::sql::RecoverableDeleteError;
use tablecraft_query::{DeleteError, QueryError};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{deletes, joined_update};

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing image byte"
    );
}

#[test]
fn correlated_delete_review_save_ack_reopen_preserves_independent_after_and_siblings() {
    for (sql, removed) in [
        (
            "DELETE t.* FROM Table1 AS t WHERE EXISTS (SELECT c.Id FROM Changes AS c WHERE c.Id=t.ID AND c.Score=5)",
            &[0][..],
        ),
        (
            "DELETE FROM Table1 WHERE ID IN (SELECT Id FROM Changes WHERE Score=5)",
            &[0][..],
        ),
        // An actual source NULL makes every nonmatching NOT IN result UNKNOWN.
        (
            "DELETE FROM Table1 WHERE ID NOT IN (SELECT Score FROM Table1)",
            &[][..],
        ),
    ] {
        let source = joined_update::source();
        let before = source.bytes;
        let expected = deletes::after(&before, source.target_definition, removed);
        let (_directory, path, worker, revision) = opened(&before);
        let (target, _) = inspect(&worker, revision, sql);
        let (stale, _) = inspect(&worker, revision, sql);
        let Response::QueryDeleteReviewed { review, capability } = request(
            &worker,
            Command::ReviewQueryDelete {
                target,
                parameters: Vec::new(),
            },
        )
        .unwrap() else {
            panic!("expected real source-bound predicate DELETE review");
        };
        assert_eq!(review.table, "Table1");
        assert_eq!(
            (review.matched_rows, review.deleted_rows),
            (removed.len(), removed.len())
        );
        assert_image(&std::fs::read(&path).unwrap(), &before);
        assert!(!crate::journal_path(&path).exists());
        let saves = SaveWorker::new(|| {});
        let id = saves
            .start_query_delete(capability, crate::journal_path(&path))
            .unwrap();
        let SaveOutcome::Saved { receipt } = terminal(&saves).outcome else {
            panic!("expected completed native DELETE save");
        };
        assert_eq!(
            (receipt.matched_rows, receipt.changed_rows),
            (Some(removed.len()), Some(removed.len()))
        );
        assert!(receipt.schema_change.is_none());
        assert!(saves.acknowledge(id));
        assert_image(&std::fs::read(&path).unwrap(), &expected);
        assert!(!crate::journal_path(&path).exists());
        request(&worker, open(&path, "")).unwrap();
        assert!(matches!(
            request(
                &worker,
                Command::ReviewQueryDelete {
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
            panic!("expected fresh survivors");
        };
        assert_eq!(columns[1].native_number, Some(1));
        assert_eq!(rows.len(), 2 - removed.len());
        let last = rows.last().unwrap();
        assert_eq!(last[0].text, "2");
        assert_eq!(last[1].text, "NULL");
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Page {
                table: "Changes".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("expected unchanged read source");
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][1].text, "5");
        assert_eq!(rows[1][1].text, "7");
        assert_image(&std::fs::read(&path).unwrap(), &expected);
    }
}

#[test]
fn late_read_error_and_unsupported_subquery_create_no_review_or_journal() {
    let before = joined_update::source().bytes;
    let (_directory, path, worker, revision) = opened(&before);
    for (sql, expected) in [
        (
            "DELETE FROM Table1 WHERE EXISTS (SELECT Id FROM Changes WHERE 10 \\ (Score-7)>0)",
            QueryError::Evaluation(tablecraft_expressions::EvalError::DivisionByZero),
        ),
        (
            "DELETE FROM Table1 WHERE ID=(SELECT Id FROM Changes)",
            QueryError::Unsupported,
        ),
    ] {
        let (target, _) = inspect(&worker, revision, sql);
        assert!(
            matches!(request(&worker, Command::ReviewQueryDelete { target, parameters: Vec::new() }), Err(Error::QueryDelete(RecoverableDeleteError::Plan(DeleteError::Query(actual)))) if actual==expected)
        );
        assert_image(&std::fs::read(&path).unwrap(), &before);
        assert!(!crate::journal_path(&path).exists());
    }
    let (stale, _) = inspect(
        &worker,
        revision,
        "DELETE FROM Table1 WHERE ID IN (SELECT Id FROM Changes)",
    );
    request(&worker, open(&path, "")).unwrap();
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryDelete {
                target: stale,
                parameters: Vec::new()
            }
        ),
        Err(Error::StaleQuery)
    ));
    assert_image(&std::fs::read(&path).unwrap(), &before);
}
