//! Populated native DROP preserves records through real context-menu and save widgets.
use super::{Entry, choose, inspect};
use crate::workspace::Desktop;
use crate::workspace::test_support::{app_harness, finish_reads, finish_schema_save, opened};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::{Harness, kittest::{NodeT, Queryable}};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::deleted_field::populated as raw;
use tablecraft_ui::CellKind;

fn assert_survivors(
    harness: &Harness<'_, Desktop>,
    columns: &[(String, Option<usize>)],
    rows: &[Vec<(String, CellKind)>],
) {
    let page = harness.state().state.data.as_ref().unwrap();
    assert_eq!(page.columns.len(), columns.len());
    for (column, (name, number)) in page.columns.iter().zip(columns) {
        assert_eq!(&column.name, name);
        assert_eq!(&column.native_number, number);
    }
    assert_eq!(page.rows.len(), rows.len());
    for (row, expected) in page.rows.iter().zip(rows) {
        assert_eq!(row.len(), expected.len());
        for (cell, (text, kind)) in row.iter().zip(expected) {
            assert_eq!(&cell.text, text);
            assert!(cell.kind == *kind, "Surviving field kind changed");
        }
    }
}

fn journey(case: raw::Case) {
    let before = raw::source(case);
    let column = raw::column(case);
    let after = raw::after(&before, column);
    let (_directory, path, _, mut harness) = opened(before.clone());
    let original = harness.state().state.active_object().unwrap();
    let page = harness.state().state.data.as_ref().unwrap();
    let selected = page
        .columns
        .iter()
        .position(|field| field.name == column)
        .unwrap();
    let columns = page
        .columns
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != selected)
        .map(|(_, field)| (field.name.clone(), field.native_number))
        .collect::<Vec<_>>();
    let rows = page
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .filter(|(index, _)| *index != selected)
                .map(|(_, cell)| (cell.text.clone(), cell.kind))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    inspect(&mut harness, column, Entry::Datasheet);
    choose(&mut harness, "field_drop_confirmation", "No");
    assert!(!harness.state().state.field_drop_open());
    assert!(!harness.state().editing.has_save());
    let unchanged = harness.state().state.data.as_ref().unwrap();
    assert_eq!(unchanged.rows.len(), rows.len());
    assert_eq!(unchanged.columns.len(), columns.len() + 1);
    assert_eq!(unchanged.columns[selected].name, column);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "Cancelled DROP: {case:?}"
    );
    assert_eq!(harness.state().state.active_object().unwrap(), original);
    assert!(!journal_path(&path).exists());

    inspect(&mut harness, column, Entry::Datasheet);
    choose(&mut harness, "field_drop_confirmation", "Yes");
    assert!(harness.state().state.field_drop_dirty());
    assert!(harness.state().editing.save.is_none());
    assert_eq!(
        std::fs::read(&path).unwrap(),
        before,
        "Pending DROP: {case:?}"
    );
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    finish_schema_save(&mut harness);
    assert_eq!(std::fs::read(&path).unwrap(), after, "Saved DROP: {case:?}");
    assert_eq!(&after[8 * 4096..9 * 4096], &before[8 * 4096..9 * 4096]);
    assert!(!journal_path(&path).exists());
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.field_drop_dirty());
    let current = harness.state().state.active_object().unwrap();
    assert_eq!(current.id, original.id);
    assert!(current.target.document_revision > original.target.document_revision);
    assert_survivors(&harness, &columns, &rows);
    drop(harness);

    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    assert_survivors(&reopened, &columns, &rows);
    assert_eq!(std::fs::read(&path).unwrap(), after);
    assert!(!journal_path(&path).exists());
}

#[test]
fn populated_number_tail_preserves_present_and_historical_records() {
    journey(raw::Case::NumberPresent);
}

#[test]
fn populated_number_tail_can_be_removed_when_absent_from_old_records() {
    journey(raw::Case::NumberAbsent);
}

#[test]
fn populated_text_tail_preserves_forwarded_roots_and_hidden_targets() {
    journey(raw::Case::TextPresent);
}

#[test]
fn populated_text_tail_can_be_removed_before_materialization() {
    journey(raw::Case::TextAbsent);
}

#[test]
fn populated_text_tail_retains_opaque_null_and_other_text_fields() {
    journey(raw::Case::MixedText);
}

#[test]
fn populated_text_tail_retains_binary_siblings() {
    journey(raw::Case::MixedBinary);
}

#[test]
fn populated_only_text_tail_can_be_removed_leaving_generated_ids() {
    journey(raw::Case::IdOnlyText);
}

#[test]
fn populated_source_promotion_does_not_allow_deleting_the_generated_indexed_id() {
    let before = raw::source(raw::CASES[0]);
    let (_directory, path, _, mut harness) = opened(before.clone());
    let original = harness.state().state.active_object().unwrap();
    inspect(&mut harness, "ID", Entry::Datasheet);
    let yes = harness.get_by_role_and_label(Role::Button, "Yes");
    assert!(yes.accesskit_node().is_disabled());
    yes.click_accesskit();
    harness.run_steps(1);
    assert!(harness.state().state.field_drop_confirmation_open());
    assert!(!harness.state().state.field_drop_dirty());
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(harness.state().state.active_object().unwrap(), original);
    assert!(!journal_path(&path).exists());
    choose(&mut harness, "field_drop_confirmation", "No");
    assert!(!harness.state().state.field_drop_open());
}
