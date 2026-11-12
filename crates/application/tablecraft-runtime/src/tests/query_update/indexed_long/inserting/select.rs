//! Complete original self/join/aggregate SELECTs precede one native INSERT.
use super::super::{assert_image, create, inspect, open, opened, request, terminal};
use super::{insert_sql, update};
use crate::{Command, Error, Response, SaveOutcome, SaveWorker, Worker};
use std::path::{Path, PathBuf};
use tablecraft_access::editing::sql::RecoverableInsertError;
use tablecraft_expressions::EvalError;
use tablecraft_query::{InsertError, QueryError};
use tablecraft_testkit::fixtures::access::secondary_index::normal::inserting as raw;

fn prepared(descending: bool) -> (tempfile::TempDir, PathBuf, Worker, u64, raw::Case) {
    let case = raw::source(descending);
    let source = &case.updated.creation;
    let (directory, path, worker, revision) = opened(&source.before);
    let revision = create(&worker, &path, revision, source);
    let sql = format!(
        "UPDATE [{}] SET [{}]=CLng(Nz([{}],0)+1)",
        source.table, source.column, source.column
    );
    let revision = update(&worker, &path, revision, &sql, &case.updated.expected);
    let expected = raw::after(&case.updated, &[Some(77)]);
    insert_sql(
        &worker,
        &path,
        revision,
        "INSERT INTO Archive (CopiedScore) VALUES (77)",
        &expected,
    );
    let Response::Opened {
        document_revision, ..
    } = request(&worker, open(&path, "")).unwrap()
    else {
        panic!("fresh original relation after actual TCNA11 save");
    };
    (directory, path, worker, document_revision, case)
}

fn apply(
    worker: &Worker,
    path: &Path,
    revision: u64,
    sql: &str,
    values: &[Option<i32>],
    expected: &[u8],
) {
    let before = std::fs::read(path).unwrap();
    let (target, _) = inspect(worker, revision, sql);
    let Response::QueryInsertReviewed { review, capability } = request(
        worker,
        Command::ReviewQueryInsert {
            target,
            parameters: Vec::new(),
        },
    )
    .unwrap() else {
        panic!("complete INSERT SELECT review");
    };
    assert_eq!(review.inserted_rows, values.len());
    assert_image(&std::fs::read(path).unwrap(), &before);
    assert!(!crate::journal_path(path).exists());
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_query_insert(capability, crate::journal_path(path))
        .unwrap();
    let report = terminal(&saves);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.matched_rows.is_none() && receipt.changed_rows == Some(values.len())
        && receipt.schema_change.is_none()));
    assert!(saves.acknowledge(id));
    assert_image(&std::fs::read(path).unwrap(), expected);
    request(worker, open(path, "")).unwrap();
    assert!(!crate::journal_path(path).exists());
}

#[test]
fn self_join_and_aggregate_insert_select_stage_only_the_original_saved_relation() {
    let cases: [(&str, &[Option<i32>]); 5] = [
        (
            "INSERT INTO Archive (CopiedScore) SELECT CLng(CopiedScore+1) FROM Archive ORDER BY CopiedScore",
            &[
                Some(-5),
                Some(-5),
                Some(2),
                Some(2),
                Some(2),
                Some(9),
                Some(78),
            ],
        ),
        (
            "INSERT INTO Archive (CopiedScore) SELECT t.CopiedScore FROM Archive AS t INNER JOIN Archive AS c ON t.CopiedScore=c.CopiedScore ORDER BY t.CopiedScore",
            &[
                Some(-6),
                Some(-6),
                Some(-6),
                Some(-6),
                Some(1),
                Some(1),
                Some(1),
                Some(1),
                Some(1),
                Some(1),
                Some(1),
                Some(1),
                Some(1),
                Some(8),
                Some(77),
            ],
        ),
        (
            "INSERT INTO Archive (CopiedScore) SELECT CLng(Sum(CopiedScore)) FROM Archive",
            &[Some(76)],
        ),
        (
            "INSERT INTO Archive (CopiedScore) SELECT NULL FROM Archive WHERE CopiedScore=77",
            &[None],
        ),
        (
            "INSERT INTO Archive (CopiedScore) SELECT CopiedScore FROM Archive WHERE 1=0",
            &[],
        ),
    ];
    for descending in [false, true] {
        for (sql, selected) in cases {
            let (_directory, path, worker, revision, case) = prepared(descending);
            let mut appended = vec![Some(77)];
            appended.extend_from_slice(selected);
            let expected = raw::after(&case.updated, &appended);
            apply(&worker, &path, revision, sql, selected, &expected);
            let Response::Data { rows, .. } = request(
                &worker,
                Command::Page {
                    table: "Archive".into(),
                    offset: 0,
                },
            )
            .unwrap() else {
                panic!("fresh inserted original relation");
            };
            assert_eq!(
                rows.len(),
                case.updated.creation.rows.len() + appended.len()
            );
        }
    }
}

#[test]
fn top_omitted_late_insert_select_error_creates_no_native_prefix_or_journal() {
    let (_directory, path, worker, revision, case) = prepared(false);
    let before = raw::after(&case.updated, &[Some(77)]);
    let (target, _) = inspect(
        &worker,
        revision,
        r"INSERT INTO Archive (CopiedScore) SELECT TOP 1 CLng(10 \ (77-CopiedScore)) FROM Archive ORDER BY CopiedScore",
    );
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryInsert {
                target,
                parameters: Vec::new()
            }
        ),
        Err(Error::QueryInsert(RecoverableInsertError::Plan(
            InsertError::Query(QueryError::Evaluation(EvalError::DivisionByZero))
        )))
    ));
    assert_image(&std::fs::read(&path).unwrap(), &before);
    assert!(!crate::journal_path(&path).exists());
    let Response::Data { rows, .. } = request(
        &worker,
        Command::Query {
            options: Default::default(),
            sql: "SELECT CopiedScore FROM Archive ORDER BY CopiedScore".into(),
        },
    )
    .unwrap() else {
        panic!("healthy original relation after refused staging");
    };
    assert_eq!(rows.len(), 7);
}
