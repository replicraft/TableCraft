//! Genuine saved Design identity keeps Run attached to the current typed draft.
mod close;

use super::{accept_name, name, opened, scratch, sql};
use crate::workspace::{
    Desktop,
    test_support::{finish_reads, finish_schema_save},
};
use egui::{Event, Key, Modifiers, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_ui::ObjectKind;

fn design(harness: &mut Harness<'_, Desktop>) {
    harness
        .get_by_role_and_label(Role::Button, "View")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Design View")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
}

fn run(harness: &mut Harness<'_, Desktop>, expected: &str) {
    harness
        .get_all_by_role_and_label(Role::Button, "Run query")
        .next()
        .unwrap()
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    assert_eq!(harness.state().state.selected_cell_text(), Some(expected));
}

#[test]
fn scratch_design_save_binds_real_saved_identity_and_later_run_uses_current_draft() {
    let (_directory, path, case, clock, mut harness) = opened();
    scratch(&mut harness);
    sql(&mut harness, "SELECT 99 AS Answer;");
    design(&mut harness);
    let tab = harness.state().state.active_object().unwrap();
    assert_eq!(
        harness.state().state.query_design_draft().unwrap().columns[0]
            .alias
            .as_deref(),
        Some("Answer")
    );
    name(&mut harness, false);
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    assert!(!harness.state().state.query_creation_name_open());
    assert_eq!(harness.state().state.active_object().as_ref(), Some(&tab));
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Field expression")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("Answer: 99")
    );
    assert_eq!(clock.calls(), 0);
    assert_eq!(std::fs::read(&path).unwrap(), case.before);
    name(&mut harness, false);
    accept_name(&mut harness);
    finish_schema_save(&mut harness);
    let active = harness.state().state.active_object().unwrap();
    assert_eq!(active.id, tab.id);
    let ObjectKind::SavedQuery { object_id, name } = &active.target.kind else {
        panic!("created Design must own the actual fresh saved identity");
    };
    assert_eq!(name, "Created");
    let selected = harness.state().state.selected_saved_target().unwrap();
    assert_eq!(selected.object_id, *object_id);
    assert_eq!(selected.document_revision, active.target.document_revision);
    assert_eq!(
        harness.state().state.query_design_request().unwrap().tab,
        active
    );
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Field expression")
            .accesskit_node()
            .value()
            .as_deref(),
        Some("Answer: 99")
    );
    assert_eq!(std::fs::read(&path).unwrap(), case.expected);
    assert_eq!(clock.calls(), 1);
    run(&mut harness, "99");

    design(&mut harness);
    let before = harness
        .state()
        .state
        .query_design_request()
        .unwrap()
        .draft_revision;
    harness
        .get_by_role_and_label(Role::TextInput, "Field expression")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.event(Event::Paste("Answer: 101".into()));
    harness.run_steps(2);
    assert!(
        harness
            .state()
            .state
            .query_design_request()
            .unwrap()
            .draft_revision
            > before
    );
    assert!(!harness.state().state.has_unresolved_query_design_edits());
    assert_eq!(
        harness.state().state.query_design_draft().unwrap().columns[0]
            .alias
            .as_deref(),
        Some("Answer")
    );
    run(&mut harness, "101");
    assert_eq!(harness.state().state.active_object().unwrap().id, tab.id);
    assert_eq!(clock.calls(), 1);
    assert!(!harness.state().editing.has_save());
    // The independent full native image still contains the stored constant 99;
    // returning 101 therefore cannot be a silent Run of that old definition.
    assert_eq!(std::fs::read(path).unwrap(), case.expected);
}
