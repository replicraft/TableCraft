//! Real Design commands preserve the chosen key and the separately saved record.
use super::test_support::{
    apply_and_acknowledge, field_label, review, start_save, wait_for_bound_record,
};
use crate::workspace::{
    Desktop,
    test_support::{app_harness, finish_reads, opened},
};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_blank_creation;

pub(super) fn add_field(harness: &mut Harness<'_, Desktop>, kind_key: &str, name: &str) {
    let kind = harness.state().state.localizer.text(kind_key);
    harness
        .get_by_role_and_label(Role::Button, "Add field")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, &kind)
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .type_text(name);
    harness.key_press(Key::Enter);
    harness.run_steps(2);
}

#[test]
fn real_manual_key_keyless_and_automatic_decisions_save_then_insert_without_losing_input() {
    for decision in ["manual", "No", "Yes"] {
        let (directory, path, original, mut harness) = opened(canonical_blank_creation::source());
        harness
            .get_by_role_and_label(Role::Tab, "Create")
            .click_accesskit();
        harness.run_steps(2);
        harness
            .get_by_role_and_label(Role::Button, "Table Design")
            .click_accesskit();
        harness.run_steps(2);
        assert!(harness.state().state.table_draft_design_visible());
        assert!(!harness.state().state.can_create_table_design());
        add_field(&mut harness, "table-draft-short-text", "Name");
        add_field(&mut harness, "table-draft-number", "ActualKey");
        if decision == "manual" {
            harness
                .get_by_role_and_label(Role::Button, "ActualKey")
                .click_accesskit();
            harness.run_steps(2);
            harness
                .get_by_role_and_label(Role::Button, "Primary Key")
                .click_accesskit();
            harness.run_steps(2);
        }
        super::design_tests::view(&mut harness, "ribbon-datasheet");
        for (field, text) in [("Name", "  Ada  "), ("ActualKey", "7")] {
            let label = field_label(&harness, "table-draft-record-field", field);
            harness
                .get_by_role_and_label(Role::TextInput, &label)
                .focus();
            harness.run_steps(1);
            harness
                .get_by_role_and_label(Role::TextInput, &label)
                .type_text(text);
            harness.run_steps(2);
        }
        start_save(&mut harness, "Manual");
        if decision != "manual" {
            harness.run();
            assert_eq!(std::fs::read(&path).unwrap(), original);
            assert!(!journal_path(&path).exists());
            assert!(harness.state().table_creation.is_none());
            // Cancel the first attempt, then make the explicit choice again.
            // Neither naming nor cancel is permission to write the schema.
            harness
                .get_by_role_and_label(Role::Button, "Cancel")
                .click_accesskit();
            harness.run();
            assert!(
                harness
                    .query_by_role_and_label(Role::Button, "No")
                    .is_none(),
                "Cancel must close the primary-key decision without writing"
            );
            assert!(harness.state().state.has_unsaved_table_drafts());
            assert_eq!(std::fs::read(&path).unwrap(), original);
            start_save(&mut harness, "Manual");
            harness.run();
            harness
                .get_by_role_and_label(Role::Button, decision)
                .click_accesskit();
            harness.run_steps(2);
        }
        wait_for_bound_record(&mut harness, "Manual");
        let data = harness.state().state.data.as_ref().unwrap();
        assert!(data.rows.is_empty());
        assert_eq!(data.columns.len(), if decision == "Yes" { 3 } else { 2 });
        review(&mut harness);
        assert!(harness.state().editing.insert_review.is_some());
        apply_and_acknowledge(&mut harness);
        let expected = if decision == "Yes" {
            vec!["1", "  Ada  ", "7"]
        } else {
            vec!["  Ada  ", "7"]
        };
        let row = &harness.state().state.data.as_ref().unwrap().rows[0];
        assert_eq!(
            row.iter()
                .map(|cell| cell.text.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        assert!(!journal_path(&path).exists());
        drop(harness);

        let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
        finish_reads(&mut fresh);
        let data = fresh.state().state.data.as_ref().unwrap();
        assert_eq!(data.rows.len(), 1);
        assert_eq!(
            data.rows[0]
                .iter()
                .map(|cell| cell.text.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(data.columns.len(), expected.len());
        assert!(directory.path().join("mutation.accdb").exists());
    }
}
