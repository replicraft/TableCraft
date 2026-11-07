//! Actual unsaved row input crosses two separately acknowledged native saves.
use super::test_support::{add_named_field, name_and_confirm, start_save};
use crate::workspace::Desktop;
use crate::workspace::test_support::{app_harness, finish_reads, finish_save, opened};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use tablecraft_runtime::SaveOutcome;
use tablecraft_testkit::fixtures::access::canonical_blank_creation;
use tablecraft_ui::FluentArgs;

fn pending_record(
    key: &str,
    name: &str,
    text: &str,
) -> (tempfile::TempDir, PathBuf, Harness<'static, Desktop>) {
    let (directory, path, _, mut harness) = opened(canonical_blank_creation::source());
    harness
        .get_by_role_and_label(Role::Tab, "Create")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Table")
        .click_accesskit();
    harness.run_steps(2);
    add_named_field(&mut harness, key, name);

    let mut arguments = FluentArgs::new();
    arguments.set("field", name);
    let label = harness
        .state()
        .state
        .localizer
        .format("table-draft-record-field", &arguments);
    harness
        .get_by_role_and_label(Role::TextInput, &label)
        .focus();
    harness.run_steps(1);
    harness
        .get_by_role_and_label(Role::TextInput, &label)
        .type_text(text);
    harness.run_steps(2);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, &label)
            .value()
            .as_deref(),
        Some(text)
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        canonical_blank_creation::source()
    );
    (directory, path, harness)
}

fn save_schema_and_bind_record(harness: &mut Harness<'_, Desktop>) {
    start_save(harness, "Contacts");
    wait_for_bound_record(harness);
}

fn wait_for_bound_record(harness: &mut Harness<'_, Desktop>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !harness.state().state.inline_insert_active() || harness.state().state.busy {
        assert!(
            Instant::now() < deadline,
            "first record did not bind after schema save"
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    assert!(!harness.state().state.initial_record_pending());
    assert!(!harness.state().state.has_unsaved_table_drafts());
    assert!(!harness.state().editing.has_save());
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.table, "Contacts");
    assert!(data.rows.is_empty());
}

#[test]
fn original_table_close_waits_for_both_saves_and_the_fresh_record_revision() {
    let (_directory, path, mut harness) = pending_record("table-draft-short-text", "Name", "Ada");
    let tab = harness.state().state.active_object().unwrap();
    harness.state_mut().close_object(tab);
    harness.run_steps(3);
    harness
        .get_all_by_role_and_label(Role::Button, "Save")
        .find(|node| !node.accesskit_node().is_disabled())
        .expect("enabled save in the close confirmation")
        .click_accesskit();
    harness.run_steps(2);
    name_and_confirm(&mut harness, "Contacts");
    wait_for_bound_record(&mut harness);
    assert!(harness.state().close_after_first_record.is_some());
    assert!(harness.state().state.active_object().is_some());

    review(&mut harness);
    apply_and_acknowledge(&mut harness);
    harness.run_steps(2);
    assert!(harness.state().close_after_first_record.is_none());
    assert!(harness.state().state.object_tabs().is_empty());
    drop(harness);

    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path)));
    finish_reads(&mut reopened);
    let rows = &reopened.state().state.data.as_ref().unwrap().rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][1].text, "Ada");
}

fn review(harness: &mut Harness<'_, Desktop>) {
    harness
        .get_by_role_and_label(Role::Button, "Review new row")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
}

fn apply_and_acknowledge(harness: &mut Harness<'_, Desktop>) {
    harness
        .get_by_role_and_label(Role::Button, "Apply new row to original file")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(harness);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt } if receipt.changed_rows == Some(1) && receipt.schema_change.is_none())
    );
    assert!(harness.state().state.data.as_ref().unwrap().rows.is_empty());
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
}

#[test]
fn typed_unsaved_record_survives_schema_save_and_requires_its_own_reviewed_commit() {
    let (_directory, path, mut harness) = pending_record("table-draft-short-text", "Name", "Ada");
    save_schema_and_bind_record(&mut harness);
    let schema_bytes = std::fs::read(&path).unwrap();
    assert_ne!(schema_bytes, canonical_blank_creation::source());
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Name")
            .value()
            .as_deref(),
        Some("Ada")
    );

    harness
        .input_mut()
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .events
        .push(egui::ViewportEvent::Close);
    harness.run_steps(1);
    assert!(
        harness.output().viewport_output[&egui::ViewportId::ROOT]
            .commands
            .contains(&egui::ViewportCommand::CancelClose)
    );
    assert!(harness.state().state.inline_insert_active());
    review(&mut harness);
    assert_eq!(std::fs::read(&path).unwrap(), schema_bytes);
    apply_and_acknowledge(&mut harness);
    let rows = &harness.state().state.data.as_ref().unwrap().rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0].text, "1");
    assert_eq!(rows[0][1].text, "Ada");
}

#[test]
fn invalid_row_retains_input_and_the_saved_schema_then_corrects_without_recreating_table() {
    let (_directory, path, mut harness) =
        pending_record("table-draft-long-integer", "Score", "oops");
    save_schema_and_bind_record(&mut harness);
    let schema_bytes = std::fs::read(&path).unwrap();
    review(&mut harness);
    assert!(harness.state().editing.insert_review.is_none());
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), schema_bytes);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Score")
            .value()
            .as_deref(),
        Some("oops")
    );
    harness
        .get_by_role_and_label(Role::TextInput, "Score")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Score")
        .type_text("42");
    harness.run_steps(2);
    review(&mut harness);
    apply_and_acknowledge(&mut harness);
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.rows.len(), 1);
    assert_eq!(data.rows[0][1].text, "42");
    assert!(!harness.state().state.can_create_first_table());
}
