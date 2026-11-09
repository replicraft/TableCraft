//! Saved Design blank-row authoring uses real reviews, journals and complete raw AFTERs.
use super::{raw, test_support};
use crate::workspace::Desktop;
use crate::workspace::test_support::{finish_reads, finish_schema_save};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::{canonical_creation, canonical_native17_creation};

fn select_view(harness: &mut Harness<'_, Desktop>, label: &str) {
    harness
        .get_by_role_and_label(Role::Button, "View")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, label)
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
}

#[test]
fn design_number_and_short_text_save_exact_images_and_select_the_actual_new_field() {
    for (kind, name, native_type) in [
        ("Number", "Score Value", 4),
        ("Short Text", "Display Name", 10),
    ] {
        let before = raw::source(raw::Source::IdOnly);
        let after = if kind == "Number" {
            raw::append(&before, raw::Source::IdOnly, name)
        } else {
            raw::mixed::append_text(&before, name, 255, true)
        };
        let (_directory, path, _, mut harness) = test_support::opened(before.clone());
        let original_tab = harness.state().state.active_object().unwrap();
        select_view(&mut harness, "Design view");
        assert_eq!(
            harness.state().state.schema.as_ref().unwrap().fields.len(),
            1
        );

        let input = harness.get_by_role_and_label(Role::TextInput, "New field");
        assert!(!input.accesskit_node().is_disabled());
        input.focus();
        harness.run_steps(1);
        harness
            .get_by_role_and_label(Role::TextInput, "New field")
            .type_text(name);
        harness.run_steps(2);
        harness
            .get_by_role_and_label(Role::ComboBox, "Data Type")
            .click_accesskit();
        harness.run_steps(2);
        let kind_label = harness.state().state.localizer.text(if kind == "Number" {
            "table-draft-number"
        } else {
            "table-draft-short-text"
        });
        harness
            .get_by_role_and_label(Role::Button, &kind_label)
            .click_accesskit();
        harness.run_steps(2);
        assert_eq!(harness.state().state.field_addition_name().unwrap().1, name);
        assert_eq!(
            harness
                .get_by_role_and_label(Role::TextInput, "New field")
                .value()
                .as_deref(),
            Some(name)
        );
        assert_eq!(
            harness.state().state.schema.as_ref().unwrap().fields.len(),
            1
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(!journal_path(&path).exists());

        harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
        harness.run_steps(1);
        finish_schema_save(&mut harness);
        assert_eq!(std::fs::read(&path).unwrap(), after, "{kind}");
        assert!(!journal_path(&path).exists());
        assert!(!harness.state().editing.has_save());
        assert!(!harness.state().state.field_addition_open());
        let fresh_tab = harness.state().state.active_object().unwrap();
        assert_eq!(fresh_tab.id, original_tab.id);
        assert_ne!(
            fresh_tab.target.document_revision,
            original_tab.target.document_revision
        );
        let schema = harness.state().state.schema.as_ref().unwrap();
        assert_eq!(schema.fields.len(), 2);
        assert_eq!(schema.fields[1].name, name);
        assert_eq!(schema.fields[1].number, 1);
        assert_eq!(schema.fields[1].native_type, native_type);
        assert_eq!(
            harness
                .get_by_label(name)
                .accesskit_node()
                .data()
                .is_selected(),
            Some(true),
            "only the actual witnessed field becomes selected"
        );

        select_view(&mut harness, "Datasheet view");
        let data = harness.state().state.data.as_ref().unwrap();
        assert_eq!(data.columns.len(), 2);
        assert_eq!(data.columns[1].name, name);
        assert_eq!(data.columns[1].native_number, Some(1));
        assert_eq!(
            data.source_revision,
            Some(fresh_tab.target.document_revision)
        );
        assert!(data.rows.is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), after);
        assert!(!journal_path(&path).exists());
    }
}

#[test]
fn design_add_stays_unavailable_for_nonempty_and_unqualified_readable_sources() {
    for before in [
        canonical_creation::source(canonical_creation::Case::Empty).bytes,
        canonical_native17_creation::source(
            canonical_creation::source(canonical_creation::Case::One).bytes,
        ),
    ] {
        let (_directory, path, _, mut harness) = test_support::opened(before.clone());
        select_view(&mut harness, "Design view");
        let fields = harness.state().state.schema.as_ref().unwrap().fields.len();
        assert!(
            harness
                .get_by_role_and_label(Role::TextInput, "New field")
                .accesskit_node()
                .is_disabled()
        );
        assert!(
            harness
                .get_by_role_and_label(Role::ComboBox, "Data Type")
                .accesskit_node()
                .is_disabled()
        );
        assert!(!harness.state().state.field_addition_open());
        harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
        harness.run_steps(2);
        assert!(!harness.state().editing.has_save());
        assert_eq!(
            harness.state().state.schema.as_ref().unwrap().fields.len(),
            fields
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(!journal_path(&path).exists());
    }
}
