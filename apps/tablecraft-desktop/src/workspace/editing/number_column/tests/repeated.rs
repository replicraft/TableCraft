//! Three ordinary widget saves retain source qualification across fresh revisions.
use super::*;

fn add_number(harness: &mut egui_kittest::Harness<'_, Desktop>, column: &str) {
    let tab = harness.state().state.active_object().unwrap();
    assert!(harness.state().state.number_column_available);
    assert_eq!(
        harness
            .state()
            .state
            .number_column_inspection(&tab)
            .unwrap()
            .1,
        Some(true)
    );
    assert!(harness.state().state.can_name_number_column(&tab));
    harness
        .get_by_role_and_label(Role::Button, "Click to Add")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Number")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .type_text(column);
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().state.busy || harness.state().editing.number_column.is_some() {
        assert!(
            Instant::now() < deadline,
            "repeated Number save stalled: column={column}, report={:?}, error={:?}",
            harness.state().editing.save_report(),
            harness.state().state.error
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    finish_reads(harness);
}

#[test]
fn ordinary_click_to_add_number_saves_first_second_and_third_without_reopening_manually() {
    let before = raw::source(raw::Source::IdOnly);
    let first = raw::append(&before, raw::Source::IdOnly, "Score");
    let second = raw::repeated::append(&first, "Score2");
    let third = raw::repeated::append(&second, "Third");
    let (_directory, path, _, mut harness) = test_support::opened(before);
    let original_tab = harness.state().state.active_object().unwrap();
    let mut revision = original_tab.target.document_revision;
    for (column, expected) in [("Score", first), ("Score2", second), ("Third", third)] {
        add_number(&mut harness, column);
        assert_eq!(std::fs::read(&path).unwrap(), expected);
        assert!(!journal_path(&path).exists());
        assert!(!harness.state().editing.has_save());
        assert!(!harness.state().state.number_column_open());
        assert!(!harness.state().state.number_column_source_stale());
        let fresh = harness.state().state.active_object().unwrap();
        assert_eq!(fresh.id, original_tab.id);
        assert_ne!(fresh.target.document_revision, revision);
        revision = fresh.target.document_revision;
        let data = harness.state().state.data.as_ref().unwrap();
        assert_eq!(data.columns.last().unwrap().name, column);
        assert!(data.rows.is_empty());
        assert_eq!(data.source_revision, Some(revision));
    }
    assert!(harness.state().state.error.is_none());
}
