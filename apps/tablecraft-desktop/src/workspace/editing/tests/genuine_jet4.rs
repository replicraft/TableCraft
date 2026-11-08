//! Optional genuine V2000 host scenario; no native application window is created.
use super::*;
use tablecraft_testkit::support::access::registered_jet4;

#[test]
#[ignore = "requires explicit hash-pinned TABLECRAFT_JET4_SCALAR_SOURCE"]
fn genuine_jet4_reviewed_cell_save_refreshes_and_reopens_exact_image() {
    let fixture = registered_jet4::load_from_environment();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("genuine-v2000.mdb");
    std::fs::write(&path, &fixture.before).unwrap();
    let context = egui::Context::default();
    let mut desktop = Desktop::new(context.clone(), Some(path.clone()));
    finish_reads(&mut desktop, &context);
    desktop.submit(Command::Page {
        table: "ScalarSamples".into(),
        offset: 0,
    });
    finish_reads(&mut desktop, &context);
    let target = EditTarget {
        document_revision: desktop.state.database.as_ref().unwrap().document_revision,
        table: "ScalarSamples".into(),
        column: "ID".into(),
        parent_ordinal: 0,
    };
    assert!(desktop.state.can_edit_cell(&target));
    let (_, draft_id, input) = begin_target(&mut desktop, target, "1", Some("41".into()));
    assert!(desktop.state.can_review_edit(draft_id, &input));
    desktop.review_edit(draft_id, input);
    finish_reads(&mut desktop, &context);
    let review_id = desktop.editing.review.as_ref().unwrap().0;
    assert!(desktop.state.can_apply_edit(review_id));
    assert_eq!(std::fs::read(&path).unwrap(), fixture.before);

    desktop.apply_edit(review_id);
    let report = finish_save(&mut desktop);
    let SaveOutcome::Saved { receipt } = report.outcome else {
        panic!("expected genuine Jet4 host save: {report:?}");
    };
    assert_eq!(receipt.changed_rows, Some(1));
    assert_eq!(receipt.matched_rows, Some(1));
    assert_eq!(std::fs::read(&path).unwrap(), fixture.after);
    assert!(!journal_path(&path).exists());
    desktop.acknowledge_save(report.id);
    finish_reads(&mut desktop, &context);
    let data = desktop.state.data.as_ref().unwrap();
    assert_eq!(data.table, "ScalarSamples");
    assert_eq!(data.rows[0][0].text, "41");

    // Recreate the headless host to avoid qualifying only the automatic refresh.
    drop(desktop);
    let mut reopened = Desktop::new(context.clone(), Some(path.clone()));
    finish_reads(&mut reopened, &context);
    reopened.submit(Command::Page {
        table: "ScalarSamples".into(),
        offset: 0,
    });
    finish_reads(&mut reopened, &context);
    let data = reopened.state.data.as_ref().unwrap();
    assert_eq!(data.rows.len(), 3);
    assert_eq!(data.rows[0].len(), 14);
    assert_eq!(data.rows[0][0].text, "41");
    assert_eq!(data.rows[1][0].text, "2");
    assert_eq!(data.rows[2][0].text, "3");
    assert_eq!(std::fs::read(&path).unwrap(), fixture.after);
    fixture.assert_original_unchanged();
}
