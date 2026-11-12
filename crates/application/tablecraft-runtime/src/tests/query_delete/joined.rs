//! Joined selection reuses one actual TCND review/save and original target rows.
use super::{inspect, open, opened, request, terminal};
use crate::{Command, Error, Response, SaveOutcome, SaveWorker};
use std::fs;
use tablecraft_access::editing::sql::RecoverableDeleteError;
use tablecraft_query::{DeleteError, QueryError};
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{deletes, joined_update};

fn assert_image(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len(), "complete image length");
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing complete image byte"
    );
}

#[test]
fn distinctrow_join_reviews_once_per_original_row_and_saves_independent_after() {
    for (sql, removed) in [
        // Both Changes records match one original target. They are not two
        // deletions, and distinct target rows would never merge by equal cells.
        (
            "DELETE DISTINCTROW t.* FROM Table1 AS t INNER JOIN Changes AS c ON t.ID=1",
            &[0][..],
        ),
        // The chosen r occurrence alone supplies original target handles.
        (
            "DELETE DISTINCTROW r.* FROM Table1 AS l INNER JOIN Table1 AS r ON l.ID=1 AND r.ID=2",
            &[1][..],
        ),
        // c.ID=2 produces an absent outer target. Empty membership still goes
        // through the actual complete native source and deletion checks.
        (
            "DELETE DISTINCTROW t.* FROM Changes AS c LEFT JOIN Table1 AS t ON t.ID=c.ID AND t.ID=1 WHERE c.ID=2",
            &[][..],
        ),
    ] {
        let raw = joined_update::source();
        let expected = deletes::after(&raw.bytes, raw.target_definition, removed);
        let (_directory, path, worker, revision) = opened(&raw.bytes);
        let (declined, _) = inspect(&worker, revision, sql);
        let Response::QueryDeleteReviewed { capability, .. } = request(
            &worker,
            Command::ReviewQueryDelete {
                target: declined,
                parameters: Vec::new(),
            },
        )
        .unwrap() else {
            panic!("expected actual discardable joined review");
        };
        drop(capability);
        assert_image(&fs::read(&path).unwrap(), &raw.bytes);
        assert!(!crate::journal_path(&path).exists());
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
            panic!("expected actual joined deletion review");
        };
        assert_eq!(review.table, "Table1");
        assert_eq!(review.document_revision, revision);
        assert_eq!(
            (review.matched_rows, review.deleted_rows),
            (removed.len(), removed.len())
        );
        assert_image(&fs::read(&path).unwrap(), &raw.bytes);
        assert!(!crate::journal_path(&path).exists());
        let saves = SaveWorker::new(|| {});
        let id = saves
            .start_query_delete(capability, crate::journal_path(&path))
            .unwrap();
        let SaveOutcome::Saved { receipt } = terminal(&saves).outcome else {
            panic!("expected completed TCND save");
        };
        assert_eq!(
            (receipt.matched_rows, receipt.changed_rows),
            (Some(removed.len()), Some(removed.len()))
        );
        assert!(receipt.schema_change.is_none());
        assert!(saves.acknowledge(id));
        assert!(!crate::journal_path(&path).exists());
        assert_image(&fs::read(&path).unwrap(), &expected);
        request(&worker, open(&path, "")).unwrap();
        assert!(matches!(
            request(
                &worker,
                Command::ReviewQueryDelete {
                    target: stale,
                    parameters: Vec::new(),
                },
            ),
            Err(Error::StaleQuery)
        ));
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Page {
                table: "Table1".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("expected fresh target survivors");
        };
        assert_eq!(rows.len(), 2 - removed.len());
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Page {
                table: "Changes".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("expected intact read-side records");
        };
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][1].text, "5");
        assert_eq!(rows[1][1].text, "7");
        assert_image(&fs::read(path).unwrap(), &expected);
    }
}

#[test]
fn repeated_matches_do_not_hide_late_errors_or_enable_unqualified_join_deletes() {
    let raw = joined_update::source();
    let (_directory, path, worker, revision) = opened(&raw.bytes);
    let (target, _) = inspect(
        &worker,
        revision,
        "DELETE DISTINCTROW t.* FROM Table1 AS t INNER JOIN Changes AS c ON t.ID=1 WHERE c.ID=1 OR 10 \\ (c.Score-7)>0",
    );
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryDelete {
                target,
                parameters: Vec::new()
            },
        ),
        Err(Error::QueryDelete(RecoverableDeleteError::Plan(
            DeleteError::Query(QueryError::Evaluation(
                tablecraft_expressions::EvalError::DivisionByZero
            ))
        )))
    ));
    for sql in [
        "DELETE t.* FROM Table1 AS t INNER JOIN Changes AS c ON t.ID=c.ID",
        "DELETE DISTINCTROW t.* FROM Table1 AS t,Changes AS c WHERE t.ID=c.ID",
        "DELETE DISTINCTROW t.*,c.* FROM Table1 AS t INNER JOIN Changes AS c ON t.ID=c.ID",
    ] {
        let (target, _) = inspect(&worker, revision, sql);
        assert!(
            matches!(
                request(
                    &worker,
                    Command::ReviewQueryDelete {
                        target,
                        parameters: Vec::new()
                    },
                ),
                Err(Error::QueryDelete(RecoverableDeleteError::Shape(
                    QueryError::Unsupported
                )))
            ),
            "{sql}"
        );
        assert_image(&fs::read(&path).unwrap(), &raw.bytes);
        assert!(!crate::journal_path(&path).exists());
    }
    assert_image(&fs::read(path).unwrap(), &raw.bytes);
}
