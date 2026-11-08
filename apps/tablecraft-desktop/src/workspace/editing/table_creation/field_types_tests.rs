//! Field gallery declarations survive actual review, save and fresh inspection.
use super::test_support::{add_named_field, start_save};
use crate::workspace::test_support::{finish_reads, opened};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use tablecraft_testkit::fixtures::access::canonical_blank_creation;
use tablecraft_ui::FieldPropertyValue;

#[test]
fn gallery_fields_save_with_exact_native_types_and_constant_defaults() {
    let original = canonical_blank_creation::source();
    let (_directory, path, _, mut harness) = opened(original.clone());
    harness
        .get_by_role_and_label(Role::Tab, "Create")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Table")
        .click_accesskit();
    harness.run_steps(2);

    let fields = [
        ("table-draft-short-text", "TextValue", 10, None),
        ("table-draft-number", "NumberValue", 4, Some("0")),
        ("table-draft-yes-no", "Flag", 1, Some("False")),
        ("table-draft-currency", "Amount", 5, Some("0.0000")),
        ("table-draft-large-integer", "LargeValue", 19, Some("0")),
        ("table-draft-date-time", "OccurredAt", 8, None),
    ];
    for (key, name, _, _) in fields {
        add_named_field(&mut harness, key, name);
    }
    assert_eq!(std::fs::read(&path).unwrap(), original);

    start_save(&mut harness, "TypedFields");
    finish_reads(&mut harness);
    assert!(!harness.state().state.has_unsaved_table_drafts());
    assert!(!harness.state().editing.has_save());
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.table, "TypedFields");
    assert_eq!(data.columns.len(), fields.len() + 1);
    assert!(data.rows.is_empty());

    harness.state_mut().view_object_schema("TypedFields".into());
    finish_reads(&mut harness);
    let schema = harness.state().state.schema.as_ref().unwrap();
    assert_eq!(schema.fields.len(), fields.len() + 1);
    assert_eq!(schema.fields[0].name, "ID");
    assert_eq!(schema.fields[0].native_type, 4);
    for (field, (_, name, native_type, default)) in schema.fields[1..].iter().zip(fields) {
        assert_eq!(field.name, name);
        assert_eq!(field.native_type, native_type);
        let property = field
            .properties
            .iter()
            .find(|property| property.name == "DefaultValue")
            .expect("completed default-property observation");
        match (&property.value, default) {
            (FieldPropertyValue::Absent, None) => {}
            (FieldPropertyValue::Text(actual), Some(expected)) => assert_eq!(actual, expected),
            _ => panic!("unexpected default observation for {name}"),
        }
    }
}
