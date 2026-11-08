//! Number properties cross the actual schema-save and first-record review boundaries.
use super::design_tests::view;
use super::properties_tests::{choose, choose_input, replace};
use super::test_support::{
    apply_and_acknowledge, pending_record, review, save_schema_and_bind_record,
};
use crate::workspace::{Desktop, test_support::finish_reads};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_blank_creation;
use tablecraft_ui::{CellKind, FieldPropertyValue};

fn select_number_properties(harness: &mut Harness<'_, Desktop>, size_key: &str) {
    harness
        .get_by_role_and_label(Role::Tab, "Home")
        .click_accesskit();
    harness.run_steps(2);
    view(harness, "ribbon-table-design");
    harness
        .get_by_role_and_label(Role::Button, "Value")
        .click_accesskit();
    harness.run_steps(2);
    choose(harness, "design-field-size", size_key);
}

#[test]
fn all_number_sizes_save_exact_native_declarations_and_the_original_pending_record() {
    for (size, raw, expected, native_type, storage_bytes, carrier) in [
        ("table-draft-number-byte", "255", "255", 2, 1, "Byte"),
        (
            "table-draft-number-integer",
            "-32768",
            "-32768",
            3,
            2,
            "Int16",
        ),
        (
            "design-size-long-integer",
            "2147483647",
            "2147483647",
            4,
            4,
            "Int32",
        ),
        ("table-draft-number-single", "1.5", "1.5", 6, 4, "Single"),
        ("table-draft-number-double", "9.25", "9.25", 7, 8, "Double"),
        (
            "table-draft-number-decimal",
            "1234.56",
            "1234.56",
            16,
            17,
            "Decimal",
        ),
        (
            "table-draft-number-replication-id",
            "00112233-4455-6677-8899-aabbccddeeff",
            "{00112233-4455-6677-8899-aabbccddeeff}",
            15,
            16,
            "Guid",
        ),
    ] {
        let (_directory, path, mut harness) = pending_record("table-draft-number", "Value", raw);
        let generation = harness.state().generation;
        select_number_properties(&mut harness, size);
        if carrier == "Decimal" {
            replace(&mut harness, Role::TextInput, "Precision", "6");
            replace(&mut harness, Role::TextInput, "Scale", "2");
        }
        assert_eq!(harness.state().generation, generation);
        assert_eq!(
            std::fs::read(&path).unwrap(),
            canonical_blank_creation::source()
        );
        save_schema_and_bind_record(&mut harness);
        assert_eq!(
            harness
                .get_by_role_and_label(Role::TextInput, "Value")
                .value()
                .as_deref(),
            Some(raw)
        );
        let schema_bytes = std::fs::read(&path).unwrap();
        review(&mut harness);
        assert!(harness.state().editing.insert_review.is_some(), "{carrier}");
        assert_eq!(std::fs::read(&path).unwrap(), schema_bytes);
        assert!(!journal_path(&path).exists());
        apply_and_acknowledge(&mut harness);

        let rows = &harness.state().state.data.as_ref().unwrap().rows;
        assert_eq!(rows.len(), 1, "{carrier}");
        assert_eq!(rows[0][0].text, "1");
        assert_eq!(rows[0][1].text, expected, "{carrier}");
        assert!(matches!(
            rows[0][1].kind,
            CellKind::Scalar { is_null: false, .. }
        ));
        assert_ne!(std::fs::read(&path).unwrap(), schema_bytes);
        assert!(!journal_path(&path).exists());

        harness.state_mut().view_object_schema("Contacts".into());
        finish_reads(&mut harness);
        let field = &harness.state().state.schema.as_ref().unwrap().fields[1];
        assert_eq!(field.name, "Value");
        assert_eq!(field.native_type, native_type, "{carrier}");
        assert_eq!(field.storage_bytes, storage_bytes, "{carrier}");
        assert_eq!(field.scalar_type.as_deref(), Some(carrier));
        assert_eq!(
            field.decimal_parameters,
            (carrier == "Decimal").then_some((6, 2))
        );
    }
}

#[test]
fn nonexact_decimal_input_refuses_review_without_writes_and_corrects_the_same_native_schema() {
    let (_directory, path, mut harness) = pending_record("table-draft-number", "Value", "12.345");
    select_number_properties(&mut harness, "table-draft-number-decimal");
    replace(&mut harness, Role::TextInput, "Precision", "6");
    replace(&mut harness, Role::TextInput, "Scale", "2");
    save_schema_and_bind_record(&mut harness);
    let schema_bytes = std::fs::read(&path).unwrap();
    review(&mut harness);
    assert!(harness.state().editing.insert_review.is_none());
    assert!(!harness.state().editing.has_save());
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Value")
            .value()
            .as_deref(),
        Some("12.345")
    );
    assert_eq!(std::fs::read(&path).unwrap(), schema_bytes);
    assert!(!journal_path(&path).exists());

    replace(&mut harness, Role::TextInput, "Value", "12.34");
    review(&mut harness);
    assert!(harness.state().editing.insert_review.is_some());
    assert_eq!(std::fs::read(&path).unwrap(), schema_bytes);
    apply_and_acknowledge(&mut harness);
    let rows = &harness.state().state.data.as_ref().unwrap().rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0].text, "1");
    assert_eq!(rows[0][1].text, "12.34");
    assert!(!journal_path(&path).exists());
}

#[test]
fn replication_id_absent_or_null_defaults_and_explicit_null_never_use_hidden_raw_text() {
    for (default_key, input_mode) in [
        ("table-draft-property-absent", "Omitted"),
        ("table-draft-property-null", "Omitted"),
        ("table-draft-property-absent", "NULL"),
    ] {
        let (_directory, path, mut harness) = pending_record(
            "table-draft-number",
            "Value",
            "not a GUID; hidden exact text",
        );
        choose_input(&mut harness, "Value", input_mode);
        select_number_properties(&mut harness, "table-draft-number-replication-id");
        choose(&mut harness, "design-default-value", default_key);
        save_schema_and_bind_record(&mut harness);
        let schema_bytes = std::fs::read(&path).unwrap();
        review(&mut harness);
        assert!(harness.state().editing.insert_review.is_some());
        assert_eq!(std::fs::read(&path).unwrap(), schema_bytes);
        apply_and_acknowledge(&mut harness);
        let rows = &harness.state().state.data.as_ref().unwrap().rows;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0].text, "1");
        assert!(matches!(
            rows[0][1].kind,
            CellKind::Scalar { is_null: true, .. }
        ));
        assert!(!journal_path(&path).exists());

        harness.state_mut().view_object_schema("Contacts".into());
        finish_reads(&mut harness);
        let field = &harness.state().state.schema.as_ref().unwrap().fields[1];
        assert_eq!(field.native_type, 15);
        let default = &field
            .properties
            .iter()
            .find(|property| property.name == "DefaultValue")
            .unwrap()
            .value;
        match default_key {
            "table-draft-property-absent" => assert!(matches!(default, FieldPropertyValue::Absent)),
            "table-draft-property-null" => {
                assert!(matches!(default, FieldPropertyValue::Text(text) if text == "Null"))
            }
            _ => unreachable!("closed default-mode cases"),
        }
    }
}
