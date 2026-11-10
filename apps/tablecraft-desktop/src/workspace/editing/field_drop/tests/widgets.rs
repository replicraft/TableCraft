//! Actual Datasheet/Design deletion, witnessed publication and sparse Number INSERT.
use super::{Entry, choose, inspect};
use crate::workspace::test_support::objects::{EXACT_SCRATCH, scratch};
use crate::workspace::test_support::{
    self, finish_reads, finish_schema_save, inserting::insert_omitted_fields,
};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::deleted_field as raw;
use tablecraft_ui::ObjectKind;

#[test]
fn normal_number_and_text_widgets_keep_pending_inert_then_publish_only_the_complete_drop() {
    for (case, column, entry, survivors) in [
        (
            raw::SourceCase::NumberHistory,
            "Score",
            Entry::Datasheet,
            vec![("ID", 0), ("Score2", 2)],
        ),
        (
            raw::SourceCase::MixedTextHistory,
            "Label",
            Entry::Design,
            vec![("ID", 0), ("AfterLabel", 2), ("OtherLabel", 3)],
        ),
    ] {
        let before = raw::source(case);
        let after = raw::remove_nonprimary(&before, column, raw::Variant::AppendedGroupRemoved);
        let (_directory, path, _, mut harness) = test_support::opened(before.clone());
        let table = harness.state().state.active_object().unwrap();
        let scratch = scratch(&mut harness, &table);
        let old_tabs = harness.state().state.object_tabs();

        inspect(&mut harness, column, entry);
        choose(&mut harness, "field_drop_confirmation", "No");
        assert!(!harness.state().state.field_drop_open());
        assert!(!harness.state().editing.has_save());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(!journal_path(&path).exists());

        // Restore Datasheet before the helper chooses the actual Design command.
        if matches!(entry, Entry::Design) {
            harness
                .get_by_role_and_label(Role::Button, "View")
                .click_accesskit();
            harness.run_steps(2);
            harness
                .get_by_role_and_label(Role::Button, "Datasheet view")
                .click_accesskit();
            harness.run_steps(1);
            finish_reads(&mut harness);
        }
        inspect(&mut harness, column, entry);
        choose(&mut harness, "field_drop_confirmation", "Yes");
        let (target, revision) = harness.state().state.pending_field_drop().unwrap();
        assert_eq!(target.column, column);
        assert_eq!(target.native_number, 1);
        assert_eq!(revision, 1);
        assert!(harness.state().state.field_drop_dirty());
        assert!(harness.state().editing.field_drop.is_none());
        assert!(harness.state().editing.save.is_none());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(!journal_path(&path).exists());
        match entry {
            Entry::Datasheet => assert!(
                harness
                    .state()
                    .state
                    .data
                    .as_ref()
                    .unwrap()
                    .columns
                    .iter()
                    .any(|field| field.name == column && field.native_number == Some(1))
            ),
            Entry::Design => assert!(
                harness
                    .state()
                    .state
                    .schema
                    .as_ref()
                    .unwrap()
                    .fields
                    .iter()
                    .any(|field| field.name == column && field.number == 1)
            ),
        }

        harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
        harness.run_steps(1);
        finish_schema_save(&mut harness);
        assert_eq!(std::fs::read(&path).unwrap(), after);
        assert!(!journal_path(&path).exists());
        assert!(!harness.state().state.field_drop_open());
        assert!(!harness.state().state.field_drop_dirty());
        let current = harness.state().state.active_object().unwrap();
        assert_eq!(current.id, table.id);
        assert_ne!(
            current.target.document_revision,
            table.target.document_revision
        );
        let fresh_fields = match entry {
            Entry::Datasheet => harness
                .state()
                .state
                .data
                .as_ref()
                .unwrap()
                .columns
                .iter()
                .map(|field| (field.name.as_str(), field.native_number.unwrap()))
                .collect::<Vec<_>>(),
            Entry::Design => harness
                .state()
                .state
                .schema
                .as_ref()
                .unwrap()
                .fields
                .iter()
                .map(|field| (field.name.as_str(), field.number))
                .collect::<Vec<_>>(),
        };
        assert_eq!(fresh_fields, survivors);
        if matches!(entry, Entry::Design) {
            let selected = harness.get_by_label("AfterLabel");
            assert_eq!(selected.accesskit_node().data().is_selected(), Some(true));
            assert!(
                harness
                    .query_by_role_and_label(Role::TextInput, "Field name")
                    .is_none(),
                "remaining native declarations stay read-only"
            );
        }
        let tabs = harness.state().state.object_tabs();
        assert_eq!(tabs.len(), old_tabs.len());
        for (old, new) in old_tabs.iter().zip(&tabs) {
            assert_eq!(old.id, new.id);
            assert_eq!(old.target.kind, new.target.kind);
        }
        let fresh_scratch = tabs.iter().find(|tab| tab.id == scratch.id).unwrap();
        assert_eq!(fresh_scratch.target.kind, ObjectKind::Sql);
        harness.state_mut().activate_object(fresh_scratch.clone());
        finish_reads(&mut harness);
        assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
        let fresh_table = harness
            .state()
            .state
            .object_tabs()
            .into_iter()
            .find(|tab| tab.id == table.id)
            .unwrap();
        harness.state_mut().activate_object(fresh_table.clone());
        finish_reads(&mut harness);

        if matches!(case, raw::SourceCase::NumberHistory) {
            let inserted = raw::inserting::first_number_row(&after, &[(0, Some(1)), (2, Some(0))]);
            insert_omitted_fields(
                &mut harness,
                &path,
                &after,
                fresh_table.target.document_revision,
                &["Score2"],
            );
            assert_eq!(std::fs::read(&path).unwrap(), inserted);
            let data = harness.state().state.data.as_ref().unwrap();
            assert_eq!(data.columns[0].native_number, Some(0));
            assert_eq!(data.columns[1].native_number, Some(2));
            assert_eq!(data.rows[0][0].text, "1");
            assert_eq!(data.rows[0][1].text, "0");
        }
        // A surviving Text variable-slot gap is deliberately not INSERT-qualified.
    }
}
