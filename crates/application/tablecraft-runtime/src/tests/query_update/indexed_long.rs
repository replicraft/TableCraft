//! Create a real ordinary index, then update that same saved native field atomically.
use super::{assert_image, inspect, open, opened, request, save_and_acknowledge, terminal};
use crate::{Command, Error, Response, SaveOutcome, SaveSchemaChange, SaveWorker, Worker};
use std::path::Path;
use tablecraft_access::editing::sql::RecoverableError;
use tablecraft_query::{QueryError, UpdateError};
use tablecraft_testkit::fixtures::access::secondary_index::normal::{self as raw, updating};

fn create(worker: &Worker, path: &Path, revision: u64, source: &raw::Source) -> u64 {
    let sql = format!(
        "CREATE INDEX [{}] ON [{}] ([{}] {})",
        source.index_name,
        source.table,
        source.column,
        if source.descending { "DESC" } else { "ASC" }
    );
    let (target, _) = inspect(worker, revision, &sql);
    let Response::IndexCreationReviewed { capability } =
        request(worker, Command::ReviewSchemaSql { target }).unwrap()
    else {
        panic!("actual CREATE INDEX review");
    };
    let witness = capability.reopen_witness();
    assert_image(witness.expected_bytes(), &source.expected);
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_index_creation(capability, crate::journal_path(path))
        .unwrap();
    let report = terminal(&saves);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.schema_change == Some(SaveSchemaChange::IndexCreated)
        && receipt.matched_rows.is_none() && receipt.changed_rows.is_none()));
    assert!(saves.acknowledge(id));
    assert_image(&std::fs::read(path).unwrap(), &source.expected);
    let Response::Opened {
        document_revision, ..
    } = request(worker, Command::OpenIndexCreated { witness }).unwrap()
    else {
        panic!("guarded created-index reopen");
    };
    assert!(document_revision > revision);
    document_revision
}

#[test]
fn created_indexed_long_updates_use_actual_saved_source_and_preserve_companion_bytes() {
    for descending in [false, true] {
        for case in [
            updating::incremented(descending),
            updating::protected_incremented(descending),
        ] {
            let source = &case.creation;
            let (_directory, path, worker, revision) = opened(&source.before);
            let revision = create(&worker, &path, revision, source);
            // Indexed empty and nonempty requests choose their indexed language
            // before the existing unindexed empty/batch paths. Review is inert.
            for (criterion, matched) in [("1=0", 0), ("1=1", source.rows.len())] {
                let sql = format!(
                    "UPDATE [{}] SET [{}]=[{}] WHERE {criterion}",
                    source.table, source.column, source.column
                );
                let (target, _) = inspect(&worker, revision, &sql);
                let Response::QueryUpdateReviewed { review, capability } = request(
                    &worker,
                    Command::ReviewQueryUpdate {
                        target,
                        parameters: Vec::new(),
                    },
                )
                .unwrap() else {
                    panic!("full indexed no-op qualification");
                };
                assert_eq!((review.matched_rows, review.changed_rows), (matched, 0));
                drop(capability);
                assert_image(&std::fs::read(&path).unwrap(), &source.expected);
                assert!(!crate::journal_path(&path).exists());
            }
            let sql = format!(
                "UPDATE [{}] SET [{}]=CLng(Nz([{}],0)+1)",
                source.table, source.column, source.column
            );
            let (target, _) = inspect(&worker, revision, &sql);
            let (stale, _) = inspect(&worker, revision, &sql);
            let Response::QueryUpdateReviewed { review, capability } = request(
                &worker,
                Command::ReviewQueryUpdate {
                    target,
                    parameters: Vec::new(),
                },
            )
            .unwrap() else {
                panic!("actual indexed LONG review");
            };
            assert_eq!(
                (review.matched_rows, review.changed_rows),
                (case.cells.len(), case.cells.len())
            );
            assert_image(&std::fs::read(&path).unwrap(), &source.expected);
            let receipt = save_and_acknowledge(capability, crate::journal_path(&path));
            assert_eq!(
                (receipt.matched_rows, receipt.changed_rows),
                (Some(case.cells.len()), Some(case.cells.len()))
            );
            assert!(receipt.schema_change.is_none());
            assert_image(&std::fs::read(&path).unwrap(), &case.expected);
            request(&worker, open(&path, "")).unwrap();
            assert!(matches!(
                request(
                    &worker,
                    Command::ReviewQueryUpdate {
                        target: stale,
                        parameters: Vec::new(),
                    }
                ),
                Err(Error::StaleQuery)
            ));
            let Response::Data {
                rows,
                source_revision,
                ..
            } = request(
                &worker,
                Command::Query {
                    options: Default::default(),
                    sql: format!(
                        "SELECT [{}] FROM [{}] ORDER BY [{}]",
                        source.column, source.table, source.column
                    ),
                },
            )
            .unwrap()
            else {
                panic!("updated ordered query result");
            };
            assert_eq!(source_revision, None);
            let mut expected_values = case.cells.iter().map(|cell| cell.value).collect::<Vec<_>>();
            expected_values.sort_unstable();
            assert_eq!(rows.len(), expected_values.len());
            for (row, expected) in rows.iter().zip(expected_values) {
                assert_eq!(
                    row[0].text,
                    expected.map_or_else(|| "NULL".to_string(), |value| value.to_string())
                );
            }
            assert_image(&std::fs::read(path).unwrap(), &case.expected);
        }
    }
}

#[test]
fn late_indexed_assignment_error_keeps_the_actual_created_index_and_every_original_row() {
    let case = updating::incremented(false);
    let source = &case.creation;
    let (_directory, path, worker, revision) = opened(&source.before);
    let revision = create(&worker, &path, revision, source);
    let sql = format!(
        "UPDATE [{}] SET [{}]=10 \\ (7-[{}])",
        source.table, source.column, source.column
    );
    let (target, _) = inspect(&worker, revision, &sql);
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryUpdate {
                target,
                parameters: Vec::new(),
            }
        ),
        Err(Error::QueryUpdate(RecoverableError::Plan(
            UpdateError::Query(QueryError::Evaluation(
                tablecraft_expressions::EvalError::DivisionByZero
            ))
        )))
    ));
    assert_image(&std::fs::read(&path).unwrap(), &source.expected);
    assert!(!crate::journal_path(&path).exists());
}
