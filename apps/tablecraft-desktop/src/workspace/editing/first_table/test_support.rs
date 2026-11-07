//! Application-private UI steps shared by initial-table workflow tests.
use super::Desktop;
use crate::workspace::test_support::{finish_reads, finish_save, opened};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use tablecraft_runtime::SaveOutcome;
use tablecraft_testkit::fixtures::access::canonical_blank_creation;
use tablecraft_ui::FluentArgs;

pub(super) fn add_named_field(harness: &mut Harness<'_, Desktop>, key: &str, name: &str) {
    let label = harness.state().state.localizer.text(key);
    harness
        .get_by_role_and_label(Role::Button, "Click to Add")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, &label)
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .type_text(name);
    harness.run_steps(1);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
}

pub(super) fn name_and_confirm(harness: &mut Harness<'_, Desktop>, name: &str) {
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, "Table name")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Table name")
        .type_text(name);
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "OK")
        .click_accesskit();
    harness.run_steps(1);
}

pub(super) fn start_save(harness: &mut Harness<'_, Desktop>, name: &str) {
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    name_and_confirm(harness, name);
}

pub(super) fn pending_record(
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

pub(super) fn save_schema_and_bind_record(harness: &mut Harness<'_, Desktop>) {
    start_save(harness, "Contacts");
    wait_for_bound_record(harness);
}

pub(super) fn wait_for_bound_record(harness: &mut Harness<'_, Desktop>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !harness.state().state.inline_insert_active() || harness.state().state.busy {
        assert!(
            Instant::now() < deadline,
            "first record did not bind after schema save"
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    assert!(!harness.state().state.insert_continuation_pending());
    assert!(!harness.state().state.has_unsaved_table_drafts());
    assert!(!harness.state().editing.has_save());
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.table, "Contacts");
    assert!(data.rows.is_empty());
}

pub(super) fn review(harness: &mut Harness<'_, Desktop>) {
    harness
        .get_by_role_and_label(Role::Button, "Review new row")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
}

pub(super) fn apply_and_acknowledge(harness: &mut Harness<'_, Desktop>) {
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
