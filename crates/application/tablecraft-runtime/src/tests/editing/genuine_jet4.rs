//! Optional registered genuine V2000 consumer; ordinary CI uses inert raw fixtures.
use super::super::{open, request, terminal};
use crate::{Command, EditInput, EditTarget, Response, SaveOutcome, SaveWorker, Worker};
use tablecraft_testkit::support::access::registered_jet4;

#[test]
#[ignore = "requires explicit hash-pinned TABLECRAFT_JET4_SCALAR_SOURCE"]
fn genuine_jet4_reviewed_cell_save_matches_independent_whole_image() {
    let fixture = registered_jet4::load_from_environment();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("genuine-v2000.mdb");
    std::fs::write(&path, &fixture.before).unwrap();
    let worker = Worker::new(|| {});
    let Response::Opened {
        document_revision, ..
    } = request(&worker, open(&path, "")).unwrap()
    else {
        panic!("expected genuine V2000 open");
    };
    let Response::EditDraft(draft) = request(
        &worker,
        Command::BeginEdit {
            target: EditTarget {
                document_revision,
                table: "ScalarSamples".into(),
                column: "ID".into(),
                parent_ordinal: 0,
            },
        },
    )
    .unwrap() else {
        panic!("expected genuine Jet4 edit draft");
    };
    assert_eq!(draft.original.text, "1");
    let Response::EditReviewed { review, capability } = request(
        &worker,
        Command::ReviewEdit {
            draft_id: draft.id,
            input: EditInput {
                text: "41".into(),
                is_null: false,
            },
        },
    )
    .unwrap() else {
        panic!("expected unchanged Jet4 Int32 native review");
    };
    assert_eq!(review.before.text, "1");
    assert_eq!(review.after.text, "41");
    assert_eq!(review.changed_rows, 1);
    assert_eq!(std::fs::read(&path).unwrap(), fixture.before);

    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("reviewed-save.journal");
    let id = saves.start(capability, bundle.clone()).unwrap();
    let report = terminal(&saves);
    let SaveOutcome::Saved { receipt } = report.outcome else {
        panic!("expected genuine Jet4 saved receipt: {report:?}");
    };
    assert_eq!(receipt.changed_rows, Some(1));
    assert_eq!(receipt.matched_rows, Some(1));
    assert_eq!(std::fs::read(&path).unwrap(), fixture.after);
    assert!(!bundle.exists());
    assert!(saves.acknowledge(id));

    // A new worker proves reopen rather than a retained pre-edit display cache.
    let reopened = Worker::new(|| {});
    request(&reopened, open(&path, "")).unwrap();
    let Response::Data { rows, .. } = request(
        &reopened,
        Command::Page {
            table: "ScalarSamples".into(),
            offset: 0,
        },
    )
    .unwrap() else {
        panic!("expected fresh genuine Jet4 rows");
    };
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].len(), 14);
    assert_eq!(rows[0][0].text, "41");
    assert_eq!(rows[1][0].text, "2");
    assert_eq!(rows[2][0].text, "3");
    fixture.assert_original_unchanged();
}
