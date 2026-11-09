//! Actual saved Design field names compose native proof and read-only properties.
use crate::workspace::{
    editing::field_rename::widgets::propose_name,
    test_support::{self, finish_reads, finish_schema_save},
};
use egui::accesskit::{Action, Role};
use egui_kittest::kittest::{NodeT, Queryable};
use tablecraft_runtime::{SaveWorker, journal_path};
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::SourceCase, saved_field_rename,
};
use tablecraft_ui::FieldPropertyValue;

#[test]
fn saved_design_rename_reopens_design_with_same_native_field_selected_and_other_properties_readonly()
 {
    let [before, after] = saved_field_rename::pair(SourceCase::Number);
    let (_directory, path, _, mut harness) = test_support::opened(before);
    assert_eq!(
        harness.state().state.field_rename_available,
        SaveWorker::supports_field_rename()
    );
    assert!(harness.state().state.field_rename_available);
    let original_tab = harness.state().state.active_object().unwrap();
    harness
        .get_by_role_and_label(Role::Button, "View")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Design view")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    harness.run_steps(2);
    harness.get_by_label("Data Type");
    harness.get_by_label("Field Properties");
    let schema = harness.state().state.schema.as_ref().unwrap();
    assert_eq!(schema.table, "Table1");
    let original = schema
        .fields
        .iter()
        .find(|field| field.name == "Score")
        .unwrap();
    let native_number = original.number;
    let storage = (
        original.native_type,
        original.storage_bytes,
        original.fixed,
        original.flags,
        original.extended_flags,
    );
    propose_name(&mut harness, "Score", "Total");
    let target = harness.state().state.field_rename_target().unwrap();
    assert_eq!(target.native_number, native_number);
    assert_eq!(target.column, "Score");
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
    harness.run_steps(1);
    finish_schema_save(&mut harness);

    assert_eq!(std::fs::read(&path).unwrap(), after);
    assert!(!journal_path(&path).exists());
    harness.run_steps(2);
    harness.get_by_label("Data Type");
    harness.get_by_label("Field Properties");
    let current_tab = harness.state().state.active_object().unwrap();
    assert_eq!(current_tab.id, original_tab.id);
    assert_ne!(
        current_tab.target.document_revision,
        original_tab.target.document_revision
    );
    let schema = harness.state().state.schema.as_ref().unwrap();
    let renamed = schema
        .fields
        .iter()
        .find(|field| field.name == "Total")
        .unwrap();
    assert_eq!(renamed.number, native_number);
    assert_eq!(
        (
            renamed.native_type,
            renamed.storage_bytes,
            renamed.fixed,
            renamed.flags,
            renamed.extended_flags
        ),
        storage
    );
    assert!(
        renamed
            .properties
            .iter()
            .any(|property| property.name == "Required"
                && matches!(property.value, FieldPropertyValue::Boolean(false)))
    );
    assert!(
        renamed
            .properties
            .iter()
            .any(|property| property.name == "DefaultValue"
                && matches!(&property.value, FieldPropertyValue::Text(value) if value == "0"))
    );
    assert!(schema.fields.iter().all(|field| field.name != "Score"));
    assert_eq!(
        harness
            .get_by_label("Total")
            .accesskit_node()
            .data()
            .is_selected(),
        Some(true),
        "fresh Design selection must follow the same native field, not stale row position or first field"
    );
    assert!(
        harness
            .query_by_role_and_label(Role::TextInput, "Field name")
            .is_none(),
        "completed rename must not turn the remaining Design properties into editors"
    );
    let defaults = harness
        .get_all_by_role(Role::Label)
        .filter(|node| node.value().as_deref() == Some("0"))
        .collect::<Vec<_>>();
    assert!(
        !defaults.is_empty(),
        "selected renamed field still shows its stored default"
    );
    for value in defaults {
        assert!(
            !value
                .accesskit_node()
                .data()
                .supports_action(Action::SetValue),
            "stored property observations remain read-only"
        );
    }
}
