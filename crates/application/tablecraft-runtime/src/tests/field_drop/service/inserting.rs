//! A checked DROP reopen does not bypass the existing dense INSERT source profile.
use super::{opened, request, review, terminal};
use crate::{
    Command, EditError, Error, InsertInput, InsertTarget, Response, SaveOutcome, SaveWorker,
};
use tablecraft_access::editing::journal::Error as JournalError;
use tablecraft_ace::{EditError as NativeEditError, RecoverableEditError, WriteBlocker};
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::deleted_field as raw;

#[test]
fn gapped_number_schema_after_actual_drop_refuses_omitted_insert_without_writes() {
    let before = raw::source(raw::SourceCase::NumberHistory);
    let after = raw::remove_nonprimary(&before, "Score", raw::Variant::AppendedGroupRemoved);
    let (directory, path, worker, revision) = opened(&before);
    let (_, capability) = review(&worker, revision, "Score", 1).unwrap();
    let witness = capability.reopen_witness();
    let saves = SaveWorker::new(|| {});
    let drop_bundle = directory.path().join("drop.journal");
    let id = saves
        .start_field_drop(capability, drop_bundle.clone())
        .unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Saved { .. }
    ));
    assert!(saves.acknowledge(id));
    assert!(!drop_bundle.exists());
    assert_eq!(std::fs::read(&path).unwrap(), after);

    let Response::Opened {
        document_revision: fresh,
        ..
    } = request(&worker, Command::OpenFieldDropped { witness }).unwrap()
    else {
        panic!("actual DROP must complete the full-image witness reopen first");
    };
    assert_ne!(fresh, revision);
    let Response::Data { columns, rows, .. } = request(
        &worker,
        Command::Page {
            table: "Table1".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("the fresh read session must expose the actual gapped schema");
    };
    assert!(rows.is_empty());
    assert_eq!(columns.len(), 2);
    assert_eq!(
        (columns[0].name.as_str(), columns[0].native_number),
        ("ID", Some(0))
    );
    assert_eq!(
        (columns[1].name.as_str(), columns[1].native_number),
        ("Score2", Some(2))
    );

    let Response::InsertDraft(draft) = request(
        &worker,
        Command::BeginInsert {
            target: InsertTarget {
                document_revision: fresh,
                table: "Table1".into(),
            },
        },
    )
    .unwrap() else {
        panic!("metadata-only insertion must retain the real surviving declarations");
    };
    assert_eq!(draft.fields.len(), 2);
    assert!(draft.fields[0].auto_number);
    assert_eq!(draft.fields[1].name, "Score2");
    let error = request(
        &worker,
        Command::ReviewInsert {
            draft_id: draft.id,
            inputs: vec![InsertInput::Omitted, InsertInput::Omitted],
        },
    )
    .err()
    .expect("the current dense INSERT profile must refuse the gapped declaration");
    assert!(
        matches!(
            error,
            Error::Edit(EditError::Plan(JournalError::Plan(
                RecoverableEditError::Edit(NativeEditError::Unsupported(
                    WriteBlocker::ActiveContent
                ))
            )))
        ),
        "native declaration refusal must remain typed: {error:?}"
    );
    assert!(saves.poll().is_none());
    assert!(!directory.path().join("insert.journal").exists());
    assert_eq!(std::fs::read(&path).unwrap(), after);

    let Response::Data {
        source_revision,
        rows,
        ..
    } = request(
        &worker,
        Command::Page {
            table: "Table1".into(),
            offset: 0,
        },
    )
    .unwrap()
    else {
        panic!("the qualified read session remains usable after insertion refusal");
    };
    assert_eq!(source_revision, Some(fresh));
    assert!(rows.is_empty());
}
