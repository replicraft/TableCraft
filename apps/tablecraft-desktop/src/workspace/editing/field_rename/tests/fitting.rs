//! Actual named widgets retain complete source qualification for grow and shrink.
use crate::workspace::editing::field_rename::widgets::propose_name;
use crate::workspace::test_support::{self, finish_reads, finish_schema_save};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::saved_field_rename::fitting::{self, SourceCase};

#[test]
fn normal_datasheet_and_design_grow_and_shrink_real_number_and_text_names() {
    for design in [false, true] {
        for case in [SourceCase::NumberFirst, SourceCase::TextNumberText] {
            for replacement in ["TaxAmount", "X"] {
                let original = fitting::old_name(case);
                let [before, after] = fitting::pair(case, replacement);
                let (_directory, path, _, mut harness) = test_support::opened(before.clone());
                let tab = harness.state().state.active_object().unwrap();
                if design {
                    harness
                        .get_by_role_and_label(Role::Button, "View")
                        .click_accesskit();
                    harness.run_steps(2);
                    harness
                        .get_by_role_and_label(Role::Button, "Design view")
                        .click_accesskit();
                    harness.run_steps(1);
                    finish_reads(&mut harness);
                }
                propose_name(&mut harness, original, replacement);
                assert_eq!(
                    harness.state().state.field_rename_proposal().unwrap().1,
                    replacement
                );
                assert!(harness.state().state.can_review_field_rename());
                assert_eq!(std::fs::read(&path).unwrap(), before);
                assert!(!journal_path(&path).exists());
                harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
                harness.run_steps(1);
                finish_schema_save(&mut harness);
                assert_eq!(std::fs::read(&path).unwrap(), after);
                assert!(!journal_path(&path).exists());
                assert!(!harness.state().editing.has_save());
                assert!(!harness.state().state.field_rename_open());
                let reopened = harness.state().state.active_object().unwrap();
                assert_eq!(reopened.id, tab.id);
                assert_ne!(
                    reopened.target.document_revision,
                    tab.target.document_revision
                );
                if design {
                    let schema = harness.state().state.schema.as_ref().unwrap();
                    let field = schema
                        .fields
                        .iter()
                        .find(|field| field.name == replacement)
                        .unwrap();
                    assert_eq!(field.number, 1);
                    assert_eq!(
                        harness
                            .get_by_label(replacement)
                            .accesskit_node()
                            .data()
                            .is_selected(),
                        Some(true)
                    );
                } else {
                    let data = harness.state().state.data.as_ref().unwrap();
                    let column = data
                        .columns
                        .iter()
                        .find(|column| column.name == replacement)
                        .unwrap();
                    assert_eq!(column.native_number, Some(1));
                    assert!(data.rows.is_empty());
                }
            }
        }
    }
}
