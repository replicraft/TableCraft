//! Checked DROP followed by normal sparse-generated INSERT and fresh reading.
use super::{opened, request, review, terminal};
use crate::tests::open;
use crate::{Command, InsertInput, InsertTarget, Response, SaveOutcome, SaveWorker};
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::deleted_field as raw;

#[test]
fn gapped_number_schema_after_actual_drop_inserts_stored_default_through_normal_services() {
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
    let expected_insert = raw::inserting::first_number_row(&after, &[(0, Some(1)), (2, Some(0))]);
    let Response::InsertReviewed { review, capability } = request(
        &worker,
        Command::ReviewInsert {
            draft_id: draft.id,
            inputs: vec![InsertInput::Omitted, InsertInput::Omitted],
        },
    )
    .unwrap() else {
        panic!("normal source-qualified INSERT must review the actual sparse schema");
    };
    assert_eq!(review.inserted_rows, 1);
    assert!(
        review
            .inputs
            .iter()
            .all(|input| matches!(input, InsertInput::Omitted))
    );
    assert_eq!(std::fs::read(&path).unwrap(), after, "review stays inert");

    let insert_bundle = directory.path().join("insert.journal");
    let operation = saves
        .start_insert(capability, insert_bundle.clone())
        .unwrap();
    let report = terminal(&saves);
    assert_eq!(report.id, operation);
    let SaveOutcome::Saved { receipt } = report.outcome else {
        panic!("the real sparse INSERT must commit: {report:?}");
    };
    assert_eq!(receipt.changed_rows, Some(1));
    assert!(receipt.matched_rows.is_none());
    assert!(receipt.schema_change.is_none());
    assert_eq!(std::fs::read(&path).unwrap(), expected_insert);
    assert!(saves.acknowledge(operation));
    assert!(!insert_bundle.exists());

    // A save actor never substitutes new data into the existing read snapshot.
    let Response::Data {
        rows,
        source_revision,
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
        panic!("the prior read snapshot remains available until explicit reopening");
    };
    assert_eq!(source_revision, Some(fresh));
    assert!(rows.is_empty());

    request(&worker, open(&path, "")).unwrap();
    let Response::Data {
        rows,
        columns,
        source_revision,
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
        panic!("fresh reading must expose the persisted sparse row");
    };
    assert_ne!(source_revision, Some(fresh));
    assert_eq!(columns.len(), 2);
    assert_eq!(columns[0].native_number, Some(0));
    assert_eq!(columns[1].native_number, Some(2));
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0].text, "1");
    assert_eq!(rows[0][1].text, "0");
    assert_eq!(std::fs::read(&path).unwrap(), expected_insert);
}
