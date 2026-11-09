//! Actual named widgets retain complete source qualification for grow and shrink.
use crate::workspace::editing::field_rename::widgets::propose_name;
use crate::workspace::test_support::{self, finish_reads, finish_schema_save};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use std::time::{Duration, Instant};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::inserting,
    saved_field_rename::fitting::{self, SourceCase},
};

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

#[test]
fn actual_field_rename_capacity_refusal_preserves_name_then_fresh_correction_saves_exact_after() {
    let before = fitting::near_capacity_source();
    let too_long = "A".repeat(29);
    let fitting_name = "A".repeat(28);
    let [expected_before, after] = fitting::near_capacity_pair(&fitting_name);
    assert_eq!(expected_before, before);
    let (_directory, path, _, mut harness) = test_support::opened(before.clone());
    let original_tab = harness.state().state.active_object().unwrap();
    propose_name(&mut harness, "Score", &too_long);
    assert!(
        harness.state().state.can_review_field_rename(),
        "the observed fitted profile validates shape, not physical free capacity"
    );
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().state.busy || harness.state().editing.field_rename.is_some() {
        assert!(
            Instant::now() < deadline,
            "native fitted review did not finish"
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    assert_eq!(
        harness.state().state.error.as_deref(),
        Some("fitted field rename exceeds the existing name capacity")
    );
    let (target, retained, rejected_revision) =
        harness.state().state.field_rename_proposal().unwrap();
    assert_eq!(target.column, "Score");
    assert_eq!(retained, too_long);
    assert_eq!(
        harness
            .get_by_role_and_label(Role::TextInput, "Field name")
            .value()
            .as_deref(),
        Some(too_long.as_str())
    );
    assert!(harness.state().editing.save.is_none());
    assert!(!harness.state().editing.worker.is_busy());
    assert!(!journal_path(&path).exists());
    assert_eq!(std::fs::read(&path).unwrap(), before);

    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .type_text(&fitting_name);
    harness.run_steps(2);
    let (_, corrected, revision) = harness.state().state.field_rename_proposal().unwrap();
    assert_eq!(corrected, fitting_name);
    assert!(revision > rejected_revision);
    assert!(harness.state().state.can_review_field_rename());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    finish_schema_save(&mut harness);
    assert_eq!(std::fs::read(&path).unwrap(), after);
    assert!(!journal_path(&path).exists());
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.field_rename_open());
    let fresh_tab = harness.state().state.active_object().unwrap();
    assert_eq!(fresh_tab.id, original_tab.id);
    assert_ne!(
        fresh_tab.target.document_revision,
        original_tab.target.document_revision
    );
    let column = harness
        .state()
        .state
        .data
        .as_ref()
        .unwrap()
        .columns
        .iter()
        .find(|column| column.name == fitting_name)
        .unwrap();
    assert_eq!(column.native_number, Some(1));
}

#[test]
fn fitted_field_rename_then_normal_omitted_row_insert_keeps_native_names_and_preserved_history() {
    for replacement in ["TaxAmount", "X"] {
        let [before, after] = fitting::pair(SourceCase::NumberFirst, replacement);
        let (_directory, path, _, mut harness) = test_support::opened(before);
        propose_name(&mut harness, "Score", replacement);
        harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
        harness.run_steps(1);
        finish_schema_save(&mut harness);
        assert_eq!(std::fs::read(&path).unwrap(), after);
        let revision = harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .document_revision;
        crate::workspace::test_support::inserting::insert_omitted_fields(
            &mut harness,
            &path,
            &after,
            revision,
            &[replacement],
        );
        let expected = inserting::first_number_row(&after, &[(0, Some(1)), (1, Some(0))]);
        assert_eq!(std::fs::read(&path).unwrap(), expected);
    }
}
