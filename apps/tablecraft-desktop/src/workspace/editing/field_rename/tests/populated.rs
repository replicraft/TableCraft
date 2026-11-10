//! Normal header rename on actual records, followed by a separate file Open.
use crate::workspace::Desktop;
use crate::workspace::editing::field_rename::widgets::propose_name;
use crate::workspace::test_support::{app_harness, finish_reads, finish_schema_save, opened};
use egui::{Key, Modifiers};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::inserting,
    saved_field_rename::fitting::{self, SourceCase},
};

#[test]
fn normal_populated_field_rename_preserves_the_row_index_and_generator_through_fresh_open() {
    for replacement in ["TaxAmount", "X"] {
        let [empty, renamed] = fitting::pair(SourceCase::NumberFirst, replacement);
        let before = inserting::first_number_row(&empty, &[(0, Some(1)), (1, Some(42))]);
        let after = inserting::first_number_row(&renamed, &[(0, Some(1)), (1, Some(42))]);
        let (_directory, path, _, mut harness) = opened(before.clone());
        assert_eq!(
            harness.state().state.data.as_ref().unwrap().rows[0][1].text,
            "42"
        );
        let original_tab = harness.state().state.active_object().unwrap();
        propose_name(&mut harness, "Score", replacement);
        assert!(harness.state().state.can_review_field_rename());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(!journal_path(&path).exists());
        harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
        harness.run_steps(1);
        finish_schema_save(&mut harness);
        assert_eq!(std::fs::read(&path).unwrap(), after);
        assert!(!journal_path(&path).exists());
        assert!(!harness.state().editing.has_save());
        let fresh_tab = harness.state().state.active_object().unwrap();
        assert_eq!(fresh_tab.id, original_tab.id);
        assert_ne!(
            fresh_tab.target.document_revision,
            original_tab.target.document_revision
        );
        let page = harness.state().state.data.as_ref().unwrap();
        assert_eq!(page.columns[1].name, replacement);
        assert_eq!(page.columns[1].native_number, Some(1));
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0][0].text, "1");
        assert_eq!(page.rows[0][1].text, "42");
        drop(harness);

        let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
        finish_reads(&mut reopened);
        let page = reopened.state().state.data.as_ref().unwrap();
        assert_eq!(page.table, "Table1");
        assert_eq!(page.columns[1].name, replacement);
        assert_eq!(page.columns[1].native_number, Some(1));
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.rows[0][0].text, "1");
        assert_eq!(page.rows[0][1].text, "42");
        assert_eq!(std::fs::read(&path).unwrap(), after);
        assert!(!journal_path(&path).exists());
    }
}
