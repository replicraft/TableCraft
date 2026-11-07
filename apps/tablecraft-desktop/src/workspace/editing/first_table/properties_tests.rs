//! Real property controls and pending row intent cross separately reviewed native saves.
use super::design_tests::view;
use super::test_support::{
    add_named_field, apply_and_acknowledge, field_label, initial_table, pending_record, review,
    save_schema_and_bind_record,
};
use crate::workspace::{Desktop, test_support::finish_reads};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_blank_creation;
use tablecraft_ui::{CellKind, Field, FieldPropertyValue};

fn property<'a>(field: &'a Field, name: &str) -> &'a FieldPropertyValue {
    &field
        .properties
        .iter()
        .find(|property| property.name == name)
        .expect("completed native property observation")
        .value
}

fn choose(harness: &mut Harness<'_, Desktop>, key: &str, choice_key: &str) {
    let label = harness.state().state.localizer.text(key);
    let choice = harness.state().state.localizer.text(choice_key);
    harness
        .get_by_role_and_label(Role::ComboBox, &label)
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, &choice)
        .click_accesskit();
    harness.run_steps(2);
}

fn replace(harness: &mut Harness<'_, Desktop>, role: Role, label: &str, text: &str) {
    harness.get_by_role_and_label(role, label).focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    if text.is_empty() {
        harness.key_press(Key::Backspace);
    } else {
        harness.get_by_role_and_label(role, label).type_text(text);
    }
    harness.run_steps(2);
}

fn select_properties(harness: &mut Harness<'_, Desktop>, field: &str) {
    harness
        .get_by_role_and_label(Role::Tab, "Home")
        .click_accesskit();
    harness.run_steps(2);
    view(harness, "ribbon-table-design");
    harness
        .get_by_role_and_label(Role::Button, field)
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::ComboBox, "Required")
            .value()
            .as_deref(),
        Some("No")
    );
    assert_eq!(
        harness
            .get_by_role_and_label(Role::ComboBox, "Allow Zero Length")
            .value()
            .as_deref(),
        Some("Yes")
    );
}

#[test]
fn property_controls_persist_native_metadata_and_omitted_first_row_uses_only_the_stored_default() {
    let (_directory, path, mut harness) =
        pending_record("table-draft-short-text", "Name", "hidden draft");
    let mode = field_label(&harness, "insertion-field-input", "Name");
    harness
        .get_by_role_and_label(Role::Button, &mode)
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::RadioButton, "Omitted")
        .click_accesskit();
    harness.run_steps(2);
    let generation = harness.state().generation;
    select_properties(&mut harness, "Name");
    replace(&mut harness, Role::TextInput, "Field Size", "12");
    choose(&mut harness, "design-required", "table-draft-property-yes");
    choose(
        &mut harness,
        "design-allow-zero-length",
        "table-draft-property-no",
    );
    choose(
        &mut harness,
        "design-default-value",
        "table-draft-property-literal",
    );
    replace(
        &mut harness,
        Role::MultilineTextInput,
        "Literal default",
        "Guest",
    );
    assert_eq!(harness.state().generation, generation);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        canonical_blank_creation::source()
    );

    save_schema_and_bind_record(&mut harness);
    let schema_bytes = std::fs::read(&path).unwrap();
    assert_ne!(schema_bytes, canonical_blank_creation::source());
    review(&mut harness);
    assert!(harness.state().editing.insert_review.is_some());
    assert_eq!(std::fs::read(&path).unwrap(), schema_bytes);
    apply_and_acknowledge(&mut harness);
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.rows.len(), 1);
    assert_eq!(data.rows[0][0].text, "1");
    assert_eq!(data.rows[0][1].text, "Guest");
    assert_ne!(std::fs::read(&path).unwrap(), schema_bytes);

    harness.state_mut().view_object_schema("Contacts".into());
    finish_reads(&mut harness);
    let fields = &harness.state().state.schema.as_ref().unwrap().fields;
    assert_eq!(fields[0].name, "ID");
    assert_eq!(fields[1].name, "Name");
    assert_eq!(fields[1].native_type, 10);
    assert_eq!(fields[1].storage_bytes, 24);
    assert!(matches!(
        property(&fields[1], "Required"),
        FieldPropertyValue::Boolean(true)
    ));
    assert!(matches!(
        property(&fields[1], "AllowZeroLength"),
        FieldPropertyValue::Boolean(false)
    ));
    assert!(
        matches!(property(&fields[1], "DefaultValue"), FieldPropertyValue::Text(text) if text == "\"Guest\"")
    );
}

#[test]
fn yes_no_checkbox_supplies_canonical_true_through_schema_save_review_and_native_row_commit() {
    let (_directory, path, mut harness) = initial_table();
    add_named_field(&mut harness, "table-draft-yes-no", "Flag");
    let flag = field_label(&harness, "table-draft-record-field", "Flag");
    harness
        .get_by_role_and_label(Role::CheckBox, &flag)
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        canonical_blank_creation::source()
    );
    save_schema_and_bind_record(&mut harness);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Flag")
            .value()
            .as_deref(),
        Some("true")
    );
    review(&mut harness);
    assert!(harness.state().editing.insert_review.is_some());
    apply_and_acknowledge(&mut harness);
    let rows = &harness.state().state.data.as_ref().unwrap().rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0].text, "1");
    assert!(matches!(
        rows[0][1].kind,
        CellKind::Scalar {
            is_null: false,
            boolean: Some(true),
            ..
        }
    ));
    assert!(!journal_path(&path).exists());
}

#[test]
fn forbidden_zero_length_value_retains_schema_and_input_then_corrects_without_recreating_table() {
    let (_directory, path, mut harness) = pending_record("table-draft-short-text", "Name", "kept");
    select_properties(&mut harness, "Name");
    replace(&mut harness, Role::TextInput, "Field Size", "8");
    choose(&mut harness, "design-required", "table-draft-property-yes");
    choose(
        &mut harness,
        "design-allow-zero-length",
        "table-draft-property-no",
    );
    save_schema_and_bind_record(&mut harness);
    let schema_bytes = std::fs::read(&path).unwrap();
    replace(&mut harness, Role::TextInput, "Name", "");
    review(&mut harness);
    assert!(harness.state().editing.insert_review.is_none());
    assert!(!harness.state().editing.has_save());
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Name")
            .value()
            .as_deref(),
        Some("")
    );
    assert_eq!(std::fs::read(&path).unwrap(), schema_bytes);
    assert!(!journal_path(&path).exists());

    replace(&mut harness, Role::TextInput, "Name", "Ada");
    review(&mut harness);
    assert!(harness.state().editing.insert_review.is_some());
    apply_and_acknowledge(&mut harness);
    let rows = &harness.state().state.data.as_ref().unwrap().rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0].text, "1");
    assert_eq!(rows[0][1].text, "Ada");
    assert!(!harness.state().state.has_unsaved_table_drafts());
    assert!(!harness.state().state.can_create_first_table());
}
