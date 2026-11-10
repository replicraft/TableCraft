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
use tablecraft_ui::FieldRenameSaveIdentity;

#[test]
fn normal_populated_field_rename_preserves_the_row_index_and_generator_through_fresh_open() {
    for replacement in ["TaxAmount", "X", "Total"] {
        let [before, after] =
            fitting::populated_number_pair(replacement, &[Some(42), None, Some(-7)]);
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
        assert_eq!(page.rows.len(), 3);
        assert_eq!(page.rows[0][0].text, "1");
        assert_eq!(page.rows[0][1].text, "42");
        assert_eq!(page.rows[1][0].text, "2");
        assert_eq!(page.rows[1][1].text, "NULL");
        assert!(matches!(
            page.rows[1][1].kind,
            tablecraft_runtime::CellKind::Scalar { is_null: true, .. }
        ));
        assert_eq!(page.rows[2][0].text, "3");
        assert_eq!(page.rows[2][1].text, "-7");
        drop(harness);

        let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
        finish_reads(&mut reopened);
        let page = reopened.state().state.data.as_ref().unwrap();
        assert_eq!(page.table, "Table1");
        assert_eq!(page.columns[1].name, replacement);
        assert_eq!(page.columns[1].native_number, Some(1));
        assert_eq!(page.rows.len(), 3);
        assert_eq!(page.rows[0][0].text, "1");
        assert_eq!(page.rows[0][1].text, "42");
        assert_eq!(page.rows[1][0].text, "2");
        assert_eq!(page.rows[1][1].text, "NULL");
        assert!(matches!(
            page.rows[1][1].kind,
            tablecraft_runtime::CellKind::Scalar { is_null: true, .. }
        ));
        assert_eq!(page.rows[2][0].text, "3");
        assert_eq!(page.rows[2][1].text, "-7");
        assert_eq!(std::fs::read(&path).unwrap(), after);
        assert!(!journal_path(&path).exists());
    }
}

#[test]
fn normal_populated_field_rename_unchanged_review_never_starts_a_save_actor() {
    let empty = fitting::source(SourceCase::NumberFirst);
    let before = inserting::first_number_row(&empty, &[(0, Some(1)), (1, Some(42))]);
    let (_directory, path, _, mut harness) = opened(before.clone());
    super::accept_review(&mut harness, "Score");
    let host = harness.state().editing.field_rename.as_ref().unwrap();
    assert!(matches!(
        host.identity,
        Some(FieldRenameSaveIdentity::UnchangedReview { .. })
    ));
    assert!(host.reopening);
    assert!(host.terminal.is_none());
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().editing.worker.is_busy());
    assert!(!journal_path(&path).exists());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    finish_schema_save(&mut harness);
    assert!(!harness.state().state.field_rename_open());
    let page = harness.state().state.data.as_ref().unwrap();
    assert_eq!(page.columns[1].name, "Score");
    assert_eq!(page.rows[0][1].text, "42");
    assert_eq!(std::fs::read(&path).unwrap(), before);
}
