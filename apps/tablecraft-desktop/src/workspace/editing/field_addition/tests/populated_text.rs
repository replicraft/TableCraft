//! Actual saved-table Short Text ADD preserves historical values and opens fresh.
use crate::workspace::Desktop;
use crate::workspace::test_support::field_addition::add_field;
use crate::workspace::test_support::{app_harness, finish_reads, opened};
use tablecraft_builders::table::FieldAdditionKind;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::populated_text;
use tablecraft_ui::CellKind;

#[test]
fn populated_short_text_add_preserves_old_records_and_reopens_with_null_new_fields() {
    let [before, after] = populated_text::pair("Notes", &[Some(42), None, Some(-7)]);
    let (_directory, path, _, mut harness) = opened(before);
    let tab = harness.state().state.active_object().unwrap();
    add_field(&mut harness, "Notes", FieldAdditionKind::ShortText);
    assert_eq!(std::fs::read(&path).unwrap(), after);
    assert!(!journal_path(&path).exists());
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.field_addition_open());
    let current = harness.state().state.active_object().unwrap();
    assert_eq!(current.id, tab.id);
    assert!(current.target.document_revision > tab.target.document_revision);
    let page = harness.state().state.data.as_ref().unwrap();
    assert_eq!(page.columns.len(), 3);
    assert_eq!(page.columns[2].name, "Notes");
    assert_eq!(page.columns[2].native_number, Some(2));
    assert_eq!(page.rows.len(), 3);
    for (index, score) in [(0, "42"), (1, "NULL"), (2, "-7")] {
        assert_eq!(page.rows[index][0].text, (index + 1).to_string());
        assert_eq!(page.rows[index][1].text, score);
        assert_eq!(page.rows[index][2].text, "NULL");
        assert!(matches!(
            page.rows[index][2].kind,
            CellKind::Scalar { is_null: true, .. }
        ));
    }
    drop(harness);

    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path)));
    finish_reads(&mut reopened);
    let page = reopened.state().state.data.as_ref().unwrap();
    assert_eq!(page.columns[2].name, "Notes");
    assert_eq!(page.rows.len(), 3);
    for (index, score) in [(0, "42"), (1, "NULL"), (2, "-7")] {
        assert_eq!(page.rows[index][0].text, (index + 1).to_string());
        assert_eq!(page.rows[index][1].text, score);
        assert_eq!(page.rows[index][2].text, "NULL");
        assert!(matches!(
            page.rows[index][2].kind,
            CellKind::Scalar { is_null: true, .. }
        ));
    }
}
