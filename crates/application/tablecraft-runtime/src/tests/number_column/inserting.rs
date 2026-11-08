//! Adding a field must compose with the ordinary New row workflow afterward.
use super::{opened, request, review, terminal};
use crate::{
    CellKind, Command, EditInput, InsertInput, InsertTarget, Response, SaveOutcome, SaveWorker,
};
use tablecraft_testkit::fixtures::access::canonical_created_number as raw;

#[test]
fn added_number_accepts_omitted_null_and_supplied_rows_after_guarded_schema_reopen() {
    let before = raw::source(raw::Source::IdOnly);
    let (directory, path, worker, revision) = opened(&before);
    let (_, capability) = review(&worker, revision, "Table1", "Score");
    let witness = capability.reopen_witness();
    let saves = SaveWorker::new(|| {});
    let operation = saves
        .start_number_column(capability, directory.path().join("column.journal"))
        .unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Saved { .. }
    ));
    assert!(saves.acknowledge(operation));

    let Response::Opened {
        document_revision: mut revision,
        ..
    } = request(&worker, Command::OpenColumnAdded { witness }).unwrap()
    else {
        panic!("the complete saved schema must reopen before inserting rows");
    };
    let cases = [
        (InsertInput::Omitted, "0", false),
        (
            InsertInput::Value(EditInput {
                text: "unused explicit NULL input".into(),
                is_null: true,
            }),
            "NULL",
            true,
        ),
        (
            InsertInput::Value(EditInput {
                text: "37".into(),
                is_null: false,
            }),
            "37",
            false,
        ),
    ];

    for (position, (score, expected, is_null)) in cases.into_iter().enumerate() {
        let Response::InsertDraft(draft) = request(
            &worker,
            Command::BeginInsert {
                target: InsertTarget {
                    document_revision: revision,
                    table: "Table1".into(),
                },
            },
        )
        .unwrap() else {
            panic!("the new Number declaration must support an ordinary insertion draft");
        };
        assert_eq!(draft.fields.len(), 2);
        assert!(draft.fields[0].auto_number);
        assert_eq!(draft.fields[1].name, "Score");

        let Response::InsertReviewed { capability, .. } = request(
            &worker,
            Command::ReviewInsert {
                draft_id: draft.id,
                inputs: vec![InsertInput::Omitted, score],
            },
        )
        .unwrap() else {
            panic!("omission, explicit NULL and a supplied value retain distinct inputs");
        };
        let operation = saves
            .start_insert(capability, directory.path().join("row.journal"))
            .unwrap();
        let report = terminal(&saves);
        assert!(
            matches!(report.outcome, SaveOutcome::Saved { .. }),
            "{report:?}"
        );
        assert!(saves.acknowledge(operation));

        let Response::Opened {
            document_revision, ..
        } = request(&worker, super::open(&path, "")).unwrap()
        else {
            panic!("the persisted row must reopen under a fresh document revision");
        };
        revision = document_revision;
        let Response::Data { rows, .. } = request(
            &worker,
            Command::Page {
                table: "Table1".into(),
                offset: 0,
            },
        )
        .unwrap() else {
            panic!("the ordinary datasheet presenter must read the saved rows");
        };
        assert_eq!(rows.len(), position + 1);
        let row = &rows[position];
        assert_eq!(row[0].text, (position + 1).to_string());
        assert_eq!(row[1].text, expected);
        assert!(matches!(
            row[1].kind,
            CellKind::Scalar { is_null: actual, .. } if actual == is_null
        ));
    }
}
