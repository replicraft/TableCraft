//! Historical records keep NULL when a nullable defaulted field is added.
use crate::workspace::Desktop;
use crate::workspace::test_support::field_addition::add_field;
use crate::workspace::test_support::inserting::{
    apply_omitted_fields, choose_field_mode, review_and_save_insert,
};
use crate::workspace::test_support::{app_harness, finish_reads, opened};
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use tablecraft_builders::table::FieldAdditionKind;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::populated;
use tablecraft_ui::CellKind;

#[test]
fn populated_number_add_keeps_old_nulls_and_new_omitted_values_distinct_from_explicit_null() {
    let [before, after] = populated::pair("Bonus", &[Some(42), None, Some(-7)]);
    let (_directory, path, _, mut harness) = opened(before.clone());
    let original_tab = harness.state().state.active_object().unwrap();
    assert_eq!(harness.state().state.data.as_ref().unwrap().rows.len(), 3);
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!journal_path(&path).exists());

    add_field(&mut harness, "Bonus", FieldAdditionKind::Number);
    assert_eq!(std::fs::read(&path).unwrap(), after);
    assert!(!journal_path(&path).exists());
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.field_addition_open());
    let current_tab = harness.state().state.active_object().unwrap();
    assert_eq!(current_tab.id, original_tab.id);
    assert!(current_tab.target.document_revision > original_tab.target.document_revision);
    let page = harness.state().state.data.as_ref().unwrap();
    assert_eq!(page.columns.len(), 3);
    assert_eq!(page.columns[2].name, "Bonus");
    assert_eq!(page.columns[2].native_number, Some(2));
    assert_eq!(page.rows.len(), 3);
    for (row, score) in [(0, "42"), (1, "NULL"), (2, "-7")] {
        assert_eq!(page.rows[row][0].text, (row + 1).to_string());
        assert_eq!(page.rows[row][1].text, score);
        assert_eq!(page.rows[row][2].text, "NULL");
        assert!(matches!(
            page.rows[row][2].kind,
            CellKind::Scalar { is_null: true, .. }
        ));
    }

    apply_omitted_fields(
        &mut harness,
        &path,
        &after,
        current_tab.target.document_revision,
        &["Score", "Bonus"],
    );
    let inserted = populated::append_row(&after, Some(0), Some(0));
    assert_eq!(std::fs::read(&path).unwrap(), inserted);
    let page = harness.state().state.data.as_ref().unwrap();
    assert_eq!(page.rows.len(), 4);
    assert_eq!(page.rows[3][0].text, "4");
    assert_eq!(page.rows[3][1].text, "0");
    assert_eq!(page.rows[3][2].text, "0");
    assert!(matches!(
        page.rows[3][2].kind,
        CellKind::Scalar { is_null: false, .. }
    ));

    let new = harness.get_by_role_and_label(Role::Button, "New");
    assert!(!new.accesskit_node().is_disabled());
    new.click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    choose_field_mode(&mut harness, "Score", "Omitted");
    choose_field_mode(&mut harness, "Bonus", "NULL");
    review_and_save_insert(&mut harness, &path, &inserted);
    let explicit_null = populated::append_row(&inserted, Some(0), None);
    assert_eq!(std::fs::read(&path).unwrap(), explicit_null);
    assert!(!journal_path(&path).exists());
    assert!(!harness.state().editing.has_save());
    drop(harness);

    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    let page = reopened.state().state.data.as_ref().unwrap();
    assert_eq!(page.columns[2].name, "Bonus");
    assert_eq!(page.columns[2].native_number, Some(2));
    assert_eq!(page.rows.len(), 5);
    assert!(page.rows[..3].iter().all(|row| {
        matches!(row[2].kind, CellKind::Scalar { is_null: true, .. }) && row[2].text == "NULL"
    }));
    for (row, score) in [(0, "42"), (1, "NULL"), (2, "-7"), (3, "0"), (4, "0")] {
        assert_eq!(page.rows[row][0].text, (row + 1).to_string());
        assert_eq!(page.rows[row][1].text, score);
    }
    assert_eq!(page.rows[3][2].text, "0");
    assert!(matches!(
        page.rows[3][2].kind,
        CellKind::Scalar { is_null: false, .. }
    ));
    assert_eq!(page.rows[4][2].text, "NULL");
    assert!(matches!(
        page.rows[4][2].kind,
        CellKind::Scalar { is_null: true, .. }
    ));
    assert_eq!(std::fs::read(&path).unwrap(), explicit_null);
    assert!(!journal_path(&path).exists());
}
