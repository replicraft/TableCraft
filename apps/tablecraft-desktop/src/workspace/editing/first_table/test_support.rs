//! Application-private UI steps shared by initial-table workflow tests.
use super::Desktop;
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};

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
