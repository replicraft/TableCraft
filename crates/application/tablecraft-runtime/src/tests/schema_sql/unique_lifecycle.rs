//! Real UNIQUE CREATE/write/refusal/DROP capabilities retain the complete source.
use super::{inspect, review};
use crate::tests::{open, request, stored::opened, terminal};
use crate::{
    Command, Error, InsertInput, InsertTarget, Response, SaveOutcome, SaveSchemaChange, SaveWorker,
    Worker,
};
use std::path::Path;
use tablecraft_access::editing::{journal, sql};
use tablecraft_ace::{EditError, IndexConstraintViolation, InspectError, RecoverableEditError};
use tablecraft_query::{InsertError, UpdateError};
use tablecraft_testkit::fixtures::access::secondary_index::normal::{
    inserting, unique_workflow as raw,
};

fn same(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual.iter().zip(expected).position(|(a, b)| a != b),
        None,
        "first differing byte"
    );
}

fn schema(
    worker: &Worker,
    path: &Path,
    revision: u64,
    sql: &str,
    expected: &[u8],
    change: SaveSchemaChange,
) -> u64 {
    let target = inspect(worker, revision, sql);
    let before = std::fs::read(path).unwrap();
    let response = review(worker, &target).unwrap();
    same(&std::fs::read(path).unwrap(), &before);
    assert!(!crate::journal_path(path).exists());
    let saves = SaveWorker::new(|| {});
    let (id, reopen) = match response {
        Response::IndexCreationReviewed { capability } => {
            let witness = capability.reopen_witness();
            assert!(witness.unique());
            same(witness.expected_bytes(), expected);
            assert_eq!(witness.native_number(), 0);
            let id = saves
                .start_index_creation(capability, crate::journal_path(path))
                .unwrap();
            (id, Command::OpenIndexCreated { witness })
        }
        Response::IndexDropReviewed { capability } => {
            let witness = capability.reopen_witness();
            same(witness.expected_bytes(), expected);
            let id = saves
                .start_index_drop(capability, crate::journal_path(path))
                .unwrap();
            (id, Command::OpenIndexDropped { witness })
        }
        _ => panic!("actual index schema capability"),
    };
    let report = terminal(&saves);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.schema_change == Some(change)
        && receipt.matched_rows.is_none() && receipt.changed_rows.is_none()));
    assert!(saves.acknowledge(id));
    same(&std::fs::read(path).unwrap(), expected);
    let Response::Opened {
        document_revision, ..
    } = request(worker, reopen).unwrap()
    else {
        panic!("guarded native index publication");
    };
    assert!(document_revision > revision);
    assert!(matches!(review(worker, &target), Err(Error::StaleQuery)));
    assert!(!crate::journal_path(path).exists());
    document_revision
}

fn save(
    worker: &Worker,
    path: &Path,
    response: Response,
    expected: &[u8],
    matched: Option<usize>,
    changed: usize,
) -> u64 {
    let saves = SaveWorker::new(|| {});
    let id = match response {
        Response::QueryUpdateReviewed { review, capability } => {
            assert_eq!(Some(review.matched_rows), matched);
            assert_eq!(review.changed_rows, changed);
            saves
                .start_query_update(capability, crate::journal_path(path))
                .unwrap()
        }
        Response::QueryInsertReviewed { review, capability } => {
            assert_eq!(review.inserted_rows, changed);
            saves
                .start_query_insert(capability, crate::journal_path(path))
                .unwrap()
        }
        Response::InsertReviewed { review, capability } => {
            assert!(matches!(review.inputs.as_slice(), [InsertInput::Omitted]));
            saves
                .start_insert(capability, crate::journal_path(path))
                .unwrap()
        }
        _ => panic!("actual data action capability"),
    };
    let report = terminal(&saves);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.schema_change.is_none() && receipt.matched_rows == matched
        && receipt.changed_rows == Some(changed)));
    assert!(saves.acknowledge(id));
    same(&std::fs::read(path).unwrap(), expected);
    let Response::Opened {
        document_revision, ..
    } = request(worker, open(path, "")).unwrap()
    else {
        panic!("fresh source after native data action");
    };
    assert!(!crate::journal_path(path).exists());
    document_revision
}

fn action(worker: &Worker, revision: u64, sql: &str, update: bool) -> Result<Response, Error> {
    let target = inspect(worker, revision, sql);
    let command = if update {
        Command::ReviewQueryUpdate {
            target,
            parameters: Vec::new(),
        }
    } else {
        Command::ReviewQueryInsert {
            target,
            parameters: Vec::new(),
        }
    };
    request(worker, command)
}

fn duplicate(error: Error) {
    let native = match error {
        Error::QueryInsert(sql::RecoverableInsertError::Plan(InsertError::Backend(
            journal::Error::Plan(RecoverableEditError::Edit(error)),
        ))) => error,
        Error::QueryUpdate(sql::RecoverableError::Plan(UpdateError::Backend(
            journal::Error::Plan(RecoverableEditError::Edit(error)),
        ))) => error,
        error => panic!("expected typed final-key constraint error: {error:?}"),
    };
    assert!(matches!(
        native,
        EditError::Inspect(InspectError::IndexConstraint(
            IndexConstraintViolation::DuplicateKey
        ))
    ));
}

#[test]
fn unique_create_update_insert_refusals_drop_then_same_conflicting_writes_succeed() {
    for descending in [false, true] {
        let case = raw::source(descending);
        let source = &case.updated.creation;
        let (_directory, path, worker, revision) = opened(&source.before);
        let sql = format!(
            "CREATE UNIQUE INDEX ValueUnique ON UniqueValues(CopiedScore {})",
            if descending { "DESC" } else { "ASC" }
        );
        let revision = schema(
            &worker,
            &path,
            revision,
            &sql,
            &source.expected,
            SaveSchemaChange::IndexCreated,
        );
        let response = action(
            &worker,
            revision,
            "UPDATE UniqueValues SET CopiedScore=CopiedScore+10",
            true,
        )
        .unwrap();
        let revision = save(&worker, &path, response, &case.updated.expected, Some(3), 3);
        let response = action(
            &worker,
            revision,
            "INSERT INTO UniqueValues(CopiedScore) VALUES(14)",
            false,
        )
        .unwrap();
        let revision = save(&worker, &path, response, &case.inserted, None, 1);
        let first_null = inserting::after(&case.updated, &[Some(14), None]);
        let response = action(
            &worker,
            revision,
            "INSERT INTO UniqueValues(CopiedScore) VALUES(NULL)",
            false,
        )
        .unwrap();
        let revision = save(&worker, &path, response, &first_null, None, 1);
        let response = action(
            &worker,
            revision,
            "INSERT INTO UniqueValues(CopiedScore) VALUES(NULL)",
            false,
        )
        .unwrap();
        let revision = save(&worker, &path, response, &case.nulls, None, 1);
        let Response::InsertDraft(draft) = request(
            &worker,
            Command::BeginInsert {
                target: InsertTarget {
                    document_revision: revision,
                    table: source.table.into(),
                },
            },
        )
        .unwrap() else {
            panic!("actual omitted unique nullable-field draft");
        };
        assert!(!draft.fields[0].auto_number);
        let response = request(
            &worker,
            Command::ReviewInsert {
                draft_id: draft.id,
                inputs: vec![InsertInput::Omitted],
            },
        )
        .unwrap();
        let revision = save(&worker, &path, response, &case.omitted, None, 1);

        for (sql, update) in [
            ("INSERT INTO UniqueValues(CopiedScore) VALUES(12)", false),
            (
                "UPDATE UniqueValues SET CopiedScore=12 WHERE CopiedScore=13",
                true,
            ),
            (
                "UPDATE UniqueValues SET CopiedScore=12 WHERE CopiedScore IS NOT NULL",
                true,
            ),
            (
                "INSERT INTO UniqueValues(CopiedScore) SELECT 15 UNION ALL SELECT 12",
                false,
            ),
        ] {
            duplicate(
                action(&worker, revision, sql, update)
                    .err()
                    .expect("duplicate key refusal"),
            );
            same(&std::fs::read(&path).unwrap(), &case.omitted);
            assert!(!crate::journal_path(&path).exists());
        }
        let revision = schema(
            &worker,
            &path,
            revision,
            "DROP INDEX ValueUnique ON UniqueValues",
            &case.dropped,
            SaveSchemaChange::IndexDropped,
        );
        let response = action(
            &worker,
            revision,
            "INSERT INTO UniqueValues(CopiedScore) VALUES(12)",
            false,
        )
        .unwrap();
        let revision = save(&worker, &path, response, &case.duplicate_inserted, None, 1);
        let response = action(
            &worker,
            revision,
            "UPDATE UniqueValues SET CopiedScore=12 WHERE CopiedScore=11",
            true,
        )
        .unwrap();
        save(
            &worker,
            &path,
            response,
            &case.duplicate_updated,
            Some(1),
            1,
        );
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Page {
                table: source.table.into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("fresh post-drop duplicate records");
        };
        assert_eq!(rows.len(), 8);
        assert_eq!(rows[0][0].text, "12");
        assert_eq!(rows[7][0].text, "12");
    }
}

#[test]
fn final_unique_key_swap_succeeds_and_duplicate_create_keeps_unindexed_source() {
    for descending in [false, true] {
        let swapped = raw::swapped(descending);
        let source = &swapped.creation;
        let (_directory, path, worker, revision) = opened(&source.before);
        let sql = format!(
            "CREATE UNIQUE INDEX ValueUnique ON UniqueValues(CopiedScore {})",
            if descending { "DESC" } else { "ASC" }
        );
        let revision = schema(
            &worker,
            &path,
            revision,
            &sql,
            &source.expected,
            SaveSchemaChange::IndexCreated,
        );
        let response = action(&worker, revision, "UPDATE UniqueValues SET CopiedScore=IIf(CopiedScore=1,2,IIf(CopiedScore=2,1,CopiedScore)) WHERE CopiedScore<3", true).unwrap();
        save(&worker, &path, response, &swapped.expected, Some(2), 2);
    }
    let before = raw::duplicate_before();
    let (_directory, path, worker, revision) = opened(&before);
    let target = inspect(
        &worker,
        revision,
        "CREATE UNIQUE INDEX ValueUnique ON UniqueValues(CopiedScore)",
    );
    assert!(review(&worker, &target).is_err());
    same(&std::fs::read(&path).unwrap(), &before);
    assert!(!crate::journal_path(&path).exists());
    // No failed source inspection is promoted into a usable schema capability.
    let Response::Data { rows, .. } = request(
        &worker,
        Command::Page {
            table: "UniqueValues".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("healthy original relation after CREATE refusal");
    };
    assert_eq!(rows.len(), 3);
}
