//! Initial Design edits remain one draft until separately reviewed schema/row saves.
use super::test_support::{
    apply_and_acknowledge, pending_record, review, save_schema_and_bind_record,
};
use crate::workspace::{Desktop, test_support::finish_reads};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_testkit::fixtures::access::canonical_blank_creation;
use tablecraft_ui::FluentArgs;

pub(super) fn view(harness: &mut Harness<'_, Desktop>, key: &str) {
    let label = harness.state().state.localizer.text(key);
    harness
        .get_by_role_and_label(Role::Button, "View")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, &label)
        .click_accesskit();
    harness.run_steps(2);
}

fn choose_type(harness: &mut Harness<'_, Desktop>, field: &str, key: &str) {
    let mut arguments = FluentArgs::new();
    arguments.set("field", field);
    let selector = harness
        .state()
        .state
        .localizer
        .format("table-draft-design-type", &arguments);
    let choice = harness.state().state.localizer.text(key);
    harness
        .get_by_role_and_label(Role::ComboBox, &selector)
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, &choice)
        .click_accesskit();
    harness.run_steps(2);
}

#[test]
fn design_name_and_type_changes_preserve_pending_input_through_native_saves() {
    let (_directory, path, mut harness) = pending_record("table-draft-short-text", "Value", "42");
    let generation = harness.state().generation;
    harness
        .get_by_role_and_label(Role::Tab, "Home")
        .click_accesskit();
    harness.run_steps(2);
    view(&mut harness, "ribbon-table-design");
    assert!(harness.state().state.table_draft_design_visible());

    let mut arguments = FluentArgs::new();
    arguments.set("field", "Value");
    let name = harness
        .state()
        .state
        .localizer
        .format("table-draft-design-name", &arguments);
    harness
        .get_by_role_and_label(Role::Button, &name)
        .click_accesskit();
    harness.run_steps(2);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .type_text("Score");
    harness.run_steps(1);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(2);
    choose_type(&mut harness, "Score", "design-type-number");
    view(&mut harness, "ribbon-datasheet");
    assert!(!harness.state().state.table_draft_design_visible());
    assert_eq!(harness.state().generation, generation);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        canonical_blank_creation::source()
    );
    view(&mut harness, "ribbon-table-design");
    assert!(harness.state().state.table_draft_design_visible());
    assert_eq!(harness.state().generation, generation);

    save_schema_and_bind_record(&mut harness, "Contacts");
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Score")
            .value()
            .as_deref(),
        Some("42")
    );
    review(&mut harness);
    apply_and_acknowledge(&mut harness);
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.rows.len(), 1);
    assert_eq!(data.rows[0][1].text, "42");
    harness.state_mut().view_object_schema("Contacts".into());
    finish_reads(&mut harness);
    let schema = harness.state().state.schema.as_ref().unwrap();
    assert_eq!(schema.fields[1].name, "Score");
    assert_eq!(schema.fields[1].native_type, 4);
}

#[test]
fn incompatible_yes_no_input_survives_design_and_schema_save_without_becoming_false() {
    let (_directory, path, mut harness) = pending_record("table-draft-short-text", "Flag", "maybe");
    harness
        .get_by_role_and_label(Role::Tab, "Home")
        .click_accesskit();
    harness.run_steps(2);
    view(&mut harness, "ribbon-table-design");
    choose_type(&mut harness, "Flag", "design-type-yes-no");
    view(&mut harness, "ribbon-datasheet");
    let mut arguments = FluentArgs::new();
    arguments.set("field", "Flag");
    let input = harness
        .state()
        .state
        .localizer
        .format("table-draft-record-field", &arguments);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, &input)
            .value()
            .as_deref(),
        Some("maybe")
    );
    assert!(
        harness
            .query_by_role_and_label(Role::CheckBox, &input)
            .is_none()
    );

    save_schema_and_bind_record(&mut harness, "Contacts");
    let schema_bytes = std::fs::read(&path).unwrap();
    review(&mut harness);
    assert!(harness.state().editing.insert_review.is_none());
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), schema_bytes);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Flag")
            .value()
            .as_deref(),
        Some("maybe")
    );
}
