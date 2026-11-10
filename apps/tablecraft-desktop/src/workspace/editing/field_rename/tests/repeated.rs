//! Repeated ordinary header saves retain prior names' exact inactive backing bytes.
use crate::workspace::Desktop;
use crate::workspace::editing::field_rename::widgets::propose_name;
use crate::workspace::test_support::{app_harness, finish_reads, finish_schema_save, opened};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::kittest::{NodeT, Queryable};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::saved_field_rename::fitting::{self, SourceCase};
use tablecraft_ui::CellKind;

#[test]
fn repeated_empty_field_rename_saves_each_actual_source_and_reopens_the_final_name() {
    repeated_names(false);
}

#[test]
fn repeated_populated_field_rename_preserves_nullable_rows_and_generated_history() {
    repeated_names(true);
}

fn repeated_names(populated: bool) {
    let mut expected = if populated {
        let [before, _] = fitting::populated_number_pair("TaxAmount", &[Some(42), None, Some(-7)]);
        before
    } else {
        fitting::source(SourceCase::NumberFirst)
    };
    let (_directory, path, _, mut harness) = opened(expected.clone());
    let original_tab = harness.state().state.active_object().unwrap().id;
    let mut original_name = "Score";
    for replacement in ["TaxAmount", "X", "Revenue", "Score"] {
        let revision = harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .document_revision;
        propose_name(&mut harness, original_name, replacement);
        assert!(
            harness.state().state.can_review_field_rename(),
            "{original_name:?} -> {replacement:?}, populated={populated}: \
             editable={}, dirty={}, busy={}, inspection={:?}, proposal={:?}, error={:?}, \
             rendered source/name-policy labels={:?}",
            harness.state().state.can_edit_field_rename(),
            harness.state().state.field_rename_dirty(),
            harness.state().state.busy,
            harness.state().state.pending_field_rename_inspection_id(),
            harness.state().state.field_rename_proposal(),
            harness.state().state.error,
            harness
                .get_all_by_role(Role::Label)
                .filter_map(|node| node.accesskit_node().label().map(str::to_owned))
                .collect::<Vec<_>>()
        );
        assert_eq!(std::fs::read(&path).unwrap(), expected);
        assert!(!journal_path(&path).exists());
        assert!(!harness.state().editing.worker.is_busy());
        harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
        harness.run_steps(1);
        finish_schema_save(&mut harness);
        expected = fitting::rename_current(&expected, original_name, replacement);
        assert_eq!(std::fs::read(&path).unwrap(), expected);
        assert!(!journal_path(&path).exists());
        assert!(!harness.state().editing.has_save());
        assert!(!harness.state().state.field_rename_open());
        let tab = harness.state().state.active_object().unwrap();
        assert_eq!(tab.id, original_tab);
        assert!(tab.target.document_revision > revision);
        let page = harness.state().state.data.as_ref().unwrap();
        assert_eq!(page.columns[1].name, replacement);
        assert_eq!(page.columns[1].native_number, Some(1));
        assert_eq!(page.rows.len(), if populated { 3 } else { 0 });
        if populated {
            assert_eq!(page.rows[0][0].text, "1");
            assert_eq!(page.rows[0][1].text, "42");
            assert_eq!(page.rows[1][0].text, "2");
            assert!(matches!(
                page.rows[1][1].kind,
                CellKind::Scalar { is_null: true, .. }
            ));
            assert_eq!(page.rows[2][0].text, "3");
            assert_eq!(page.rows[2][1].text, "-7");
        }
        original_name = replacement;
    }
    drop(harness);

    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    let page = reopened.state().state.data.as_ref().unwrap();
    assert_eq!(page.columns[1].name, "Score");
    assert_eq!(page.columns[1].native_number, Some(1));
    assert_eq!(page.rows.len(), if populated { 3 } else { 0 });
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(!journal_path(&path).exists());
}
