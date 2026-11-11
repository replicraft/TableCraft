//! The saved-field gallery must expose working ordinary type declarations.
use crate::workspace::test_support::opened;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use tablecraft_builders::table::FieldAdditionKind;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::fields;

#[test]
fn populated_retained_saved_field_gallery_exposes_currency_as_an_actual_declaration() {
    let source = fields::source(fields::Case::FirstNumber);
    let (_directory, path, before, mut harness) = opened(source.bytes);
    let tab = harness.state().state.active_object().unwrap();
    assert_eq!(
        harness
            .state()
            .state
            .field_addition_inspection(&tab)
            .unwrap()
            .1,
        Some(true)
    );
    assert!(
        harness
            .state()
            .state
            .can_name_field_addition(&tab, FieldAdditionKind::Number)
    );
    assert!(!harness.state().state.data.as_ref().unwrap().rows.is_empty());
    harness
        .get_by_role_and_label(Role::Button, "Click to Add")
        .click_accesskit();
    harness.run_steps(2);
    let number = harness.state().state.localizer.text("table-draft-number");
    assert!(
        !harness
            .get_by_role_and_label(Role::Button, &number)
            .accesskit_node()
            .is_disabled()
    );
    let currency = harness.state().state.localizer.text("table-draft-currency");
    let choice = harness.get_by_role_and_label(Role::Button, &currency);
    assert!(
        !choice.accesskit_node().is_disabled(),
        "Currency remains unavailable in the saved-field gallery despite a qualified populated source"
    );
    choice.click_accesskit();
    harness.run_steps(2);
    let name = harness.get_by_role_and_label(Role::TextInput, "Field name");
    assert!(!name.accesskit_node().is_disabled() && !name.accesskit_node().is_read_only());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!journal_path(&path).exists());
    assert!(harness.state().editing.save.is_none());
    assert!(!harness.state().editing.worker.is_busy());
}
