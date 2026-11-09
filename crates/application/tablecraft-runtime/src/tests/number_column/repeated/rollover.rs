//! The normal runtime pools compose one LVAL rollover with omitted-field insertion.
use super::{opened, raw, request, review, terminal};
use crate::tests::open;
use crate::{
    CellKind, Command, InsertInput, InsertTarget, Response, SaveOutcome, SaveSchemaChange,
    SaveWorker, journal_path,
};

#[test]
fn normal_number_rollover_and_omitted_new_row_survive_complete_reopens() {
    let mut before = raw::repeated::source(raw::Source::IdOnly);
    for number in 2..=13 {
        before = raw::repeated::append(&before, &format!("N{number}"));
    }
    let expected = raw::repeated::append(&before, "N14");
    assert_eq!(expected.len(), before.len() + 4096);
    assert_eq!(&before[10 * 4096..], &expected[10 * 4096..11 * 4096]);
    let (_directory, path, worker, revision) = opened(&before);
    let Response::NumberColumnAvailability { availability } = request(
        &worker,
        Command::InspectNumberColumnAvailability {
            document_revision: revision,
            table: "Table1".into(),
        },
    )
    .unwrap() else {
        panic!("expected source-bound rollover availability");
    };
    assert!(availability.available);
    let (_, capability) = review(&worker, revision, "Table1", "N14");
    let witness = capability.reopen_witness();
    assert_eq!(witness.expected_bytes(), expected);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    let saves = SaveWorker::new(|| {});
    let bundle = journal_path(&path);
    let operation = saves
        .start_number_column(capability, bundle.clone())
        .unwrap();
    let report = terminal(&saves);
    assert_eq!(report.id, operation);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.schema_change == Some(SaveSchemaChange::ColumnAdded)
            && receipt.matched_rows.is_none() && receipt.changed_rows.is_none()
            && receipt.query_change.is_none()),
        "{report:?}"
    );
    assert!(saves.acknowledge(operation));
    assert!(!bundle.exists());
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    let Response::Opened {
        document_revision, ..
    } = request(&worker, Command::OpenColumnAdded { witness }).unwrap()
    else {
        panic!("actual rollover AFTER must satisfy its full-image witness");
    };
    assert_ne!(document_revision, revision);
    let Response::InsertDraft(draft) = request(
        &worker,
        Command::BeginInsert {
            target: InsertTarget {
                document_revision,
                table: "Table1".into(),
            },
        },
    )
    .unwrap() else {
        panic!("the reopened rollover schema must support ordinary New row");
    };
    assert_eq!(draft.fields.len(), 15);
    assert!(draft.fields[0].auto_number);
    assert_eq!(draft.fields.last().unwrap().name, "N14");
    let inputs = draft.fields.iter().map(|_| InsertInput::Omitted).collect();
    let Response::InsertReviewed { capability, .. } = request(
        &worker,
        Command::ReviewInsert {
            draft_id: draft.id,
            inputs,
        },
    )
    .unwrap() else {
        panic!("all fields must retain genuine omitted input");
    };
    assert_eq!(
        std::fs::read(&path).unwrap(),
        expected,
        "review must be inert"
    );
    let operation = saves.start_insert(capability, bundle.clone()).unwrap();
    let report = terminal(&saves);
    assert_eq!(report.id, operation);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.changed_rows == Some(1) && receipt.schema_change.is_none()
            && receipt.query_change.is_none()),
        "{report:?}"
    );
    assert!(saves.acknowledge(operation));
    assert!(!bundle.exists());
    request(&worker, open(&path, "")).unwrap();
    let Response::Data { rows, .. } = request(
        &worker,
        Command::Page {
            table: "Table1".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("the actual stored row must remain readable after reopen");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].len(), 15);
    assert_eq!(rows[0][0].text, "1");
    for cell in &rows[0][1..] {
        assert_eq!(cell.text, "0");
    }
    for cell in &rows[0] {
        assert!(matches!(cell.kind, CellKind::Scalar { is_null: false, .. }));
    }
}
