//! Actual retained-table cell reviews preserve siblings and freshly reopened data.
use crate::workspace::{
    Desktop,
    test_support::{
        app_harness,
        cell_edit::{begin_replacement, review_and_save},
        finish_reads,
        objects::{EXACT_SCRATCH, scratch},
        opened,
    },
};
use egui::{Key, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::{
        current_insert,
        deleted_field::populated::{
            Case,
            readdition::{self, Target as Field},
        },
    },
    canonical_retained_add_table::{
        self as raw, Target,
        rows::{self, Input, Profile},
        updates,
    },
    canonical_table_drop::current as dropped,
};
use tablecraft_ui::CellKind;

fn open_table(harness: &mut Harness<'_, Desktop>) {
    harness
        .get_by_role_and_label(Role::Button, "Table1")
        .focus();
    harness.run_steps(1);
    harness.key_press(Key::Enter);
    harness.run_steps(1);
    finish_reads(harness);
    assert_eq!(harness.state().state.data.as_ref().unwrap().table, "Table1");
}

fn contents(harness: &Harness<'_, Desktop>) -> Vec<Vec<(String, CellKind)>> {
    harness
        .state()
        .state
        .data
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| (cell.text.clone(), cell.kind))
                .collect()
        })
        .collect()
}

fn unchanged_siblings(
    harness: &Harness<'_, Desktop>,
    original: &[Vec<(String, CellKind)>],
    column: &str,
    value: Option<&str>,
) {
    let page = harness.state().state.data.as_ref().unwrap();
    let selected = page
        .columns
        .iter()
        .position(|field| field.name == column)
        .unwrap();
    assert_eq!(page.rows.len(), original.len());
    for (position, (row, before)) in page.rows.iter().zip(original).enumerate() {
        assert_eq!(row.len(), before.len());
        for (field, (cell, (text, kind))) in row.iter().zip(before).enumerate() {
            if position == 0 && field == selected {
                assert_eq!(cell.text, value.unwrap_or("NULL"));
                assert!(
                    matches!(cell.kind, CellKind::Scalar { is_null, .. } if is_null == value.is_none())
                );
            } else {
                assert_eq!(&cell.text, text);
                assert!(cell.kind == *kind);
            }
        }
    }
}

#[test]
fn retained_number_cell_changes_and_cancel_preserve_other_rows_and_generated_history() {
    let old = dropped::source(
        Case::NumberPresent,
        Field::ShortText,
        true,
        dropped::History::Inserted,
    );
    let retired = dropped::after(&old);
    let mut bytes = raw::desktop::append(&retired, "Table1", Target::GeneratedNumber);
    for input in [Input::Number(17), Input::Null, Input::Number(-7)] {
        bytes = rows::append(&bytes, 11, Profile::DesktopNumber, input);
    }
    bytes = raw::append(&bytes, "Other", Target::GeneratedText);
    let (_directory, path, _, mut harness) = opened(bytes.clone());
    let table = harness.state().state.active_object().unwrap();
    scratch(&mut harness, &table);
    let original = contents(&harness);
    let mut prior = bytes;
    let mut original_null = false;
    for (value, text) in [(Some(123), Some("123")), (None, None), (Some(0), Some("0"))] {
        let expected = updates::number(&prior, 11, 0, value);
        begin_replacement(&mut harness, 0, "Score", original_null, text);
        review_and_save(&mut harness, &path, &prior, &expected);
        unchanged_siblings(&harness, &original, "Score", text);
        assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
        prior = expected;
        original_null = value.is_none();
    }
    begin_replacement(&mut harness, 1, "Score", true, Some("99"));
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(!harness.state().state.edit_dialog_open());
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), prior);
    assert!(!journal_path(&path).exists());
    drop(harness);
    let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut fresh);
    open_table(&mut fresh);
    unchanged_siblings(&fresh, &original, "Score", Some("0"));
    assert_eq!(std::fs::read(path).unwrap(), prior);
}

#[test]
fn retained_text_growth_empty_null_and_forwarded_repeat_save_at_actual_second_table_pages() {
    let old = dropped::source(
        Case::MixedBinary,
        Field::ShortText,
        true,
        dropped::History::Deleted,
    );
    let retired = dropped::after(&old);
    let first = raw::append(&retired, "Other", Target::GeneratedNumber);
    let first = rows::append(&first, 11, Profile::NativeNumber, Input::Number(99));
    let mut bytes = raw::desktop::append(&first, "Table1", Target::GeneratedText);
    for input in [
        Input::Text("first"),
        Input::Omitted,
        Input::Text("opaque backing"),
    ] {
        bytes = rows::append(&bytes, 14, Profile::DesktopText, input);
    }
    bytes = updates::with_opaque_text_null(&bytes, 14, 2);
    let (_directory, path, _, mut harness) = opened(bytes.clone());
    open_table(&mut harness);
    let table = harness.state().state.active_object().unwrap();
    scratch(&mut harness, &table);
    let original = contents(&harness);
    let mut prior = bytes;
    let mut original_null = false;
    for value in [
        Some("日本語🦀 a longer replacement"),
        Some("x"),
        Some(""),
        None,
        Some("a much longer second growth 日本語🦀 repeated"),
    ] {
        let expected = updates::text(&prior, 14, 0, value);
        begin_replacement(&mut harness, 0, "Name", original_null, value);
        review_and_save(&mut harness, &path, &prior, &expected);
        unchanged_siblings(&harness, &original, "Name", value);
        assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
        prior = expected;
        original_null = value.is_none();
    }
    drop(harness);
    let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut fresh);
    open_table(&mut fresh);
    unchanged_siblings(
        &fresh,
        &original,
        "Name",
        Some("a much longer second growth 日本語🦀 repeated"),
    );
    assert_eq!(std::fs::read(&path).unwrap(), prior);
    assert!(!journal_path(&path).exists());
}

#[test]
fn original_sparse_text_cell_changes_still_save_after_appending_a_live_sibling() {
    let mut original = current_insert::source(Case::MixedBinary, Field::ShortText, true, true);
    let mut prior = raw::desktop::append(&original, "More", Target::GeneratedNumber);
    let (_directory, path, _, mut harness) = opened(prior.clone());
    let table = harness.state().state.active_object().unwrap();
    scratch(&mut harness, &table);
    let siblings = contents(&harness);
    let mut original_null = false;
    for value in [
        Some("a longer retained sparse replacement 日本語🦀"),
        Some(""),
        None,
        Some("refilled"),
    ] {
        let changed = readdition::replace_text(&original, 0, value);
        let expected = raw::desktop::append(&changed, "More", Target::GeneratedNumber);
        begin_replacement(&mut harness, 0, "CurrentField", original_null, value);
        review_and_save(&mut harness, &path, &prior, &expected);
        unchanged_siblings(&harness, &siblings, "CurrentField", value);
        assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
        original = changed;
        prior = expected;
        original_null = value.is_none();
    }
    drop(harness);
    let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut fresh);
    open_table(&mut fresh);
    unchanged_siblings(&fresh, &siblings, "CurrentField", Some("refilled"));
    assert_eq!(std::fs::read(&path).unwrap(), prior);
    assert!(!journal_path(&path).exists());
}
