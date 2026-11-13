//! Actual DROP, re-ADD and first-fill cycles never revive retired field values.
use super::{Entry, choose, inspect};
use crate::workspace::test_support::objects::{EXACT_SCRATCH, scratch};
use crate::workspace::Desktop;
use crate::workspace::test_support::cell_edit::{begin_replacement, review_and_save};
use crate::workspace::test_support::field_addition::add_field;
use crate::workspace::test_support::{app_harness, finish_reads, finish_schema_save, opened};
use egui::{Key, Modifiers};
use egui_kittest::Harness;
use tablecraft_builders::table::FieldAdditionKind;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_created_number::mixed::after_off::deleted_field::populated::{self as raw, readdition::{self, Target}};
use tablecraft_ui::{CellKind, ObjectKind};

fn assert_rows(
    harness: &Harness<'_, Desktop>,
    columns: &[(String, Option<usize>)],
    survivors: &[Vec<(String, CellKind)>],
    values: &[Option<String>],
    target: Target,
    number: usize,
) {
    let page = harness.state().state.data.as_ref().unwrap();
    assert_eq!(page.columns.len(), columns.len() + 1);
    for (field, (name, native_number)) in page.columns.iter().zip(columns) {
        assert_eq!(&field.name, name);
        assert_eq!(&field.native_number, native_number);
    }
    let added = page.columns.last().unwrap();
    assert_eq!(added.name, target.name());
    assert_eq!(added.native_number, Some(number));
    assert_eq!(page.rows.len(), survivors.len());
    assert_eq!(values.len(), survivors.len());
    for ((row, original), value) in page.rows.iter().zip(survivors).zip(values) {
        assert_eq!(row.len(), original.len() + 1);
        for (cell, (text, kind)) in row.iter().zip(original) {
            assert_eq!(&cell.text, text);
            assert!(cell.kind == *kind, "Surviving field kind changed");
        }
        let added = row.last().unwrap();
        assert_eq!(added.text, value.as_deref().unwrap_or("NULL"));
        assert!(
            matches!(added.kind, CellKind::Scalar { is_null, .. } if is_null == value.is_none())
        );
    }
}

fn journey(case: raw::Case, target: Target) {
    let before = raw::source(case);
    let dropped = raw::after(&before, raw::column(case));
    let (_directory, path, _, mut harness) = opened(before);
    let original_tab = harness.state().state.active_object().unwrap();
    scratch(&mut harness, &original_tab);
    inspect(&mut harness, raw::column(case), Entry::Datasheet);
    choose(&mut harness, "field_drop_confirmation", "Yes");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    finish_schema_save(&mut harness);
    assert_eq!(std::fs::read(&path).unwrap(), dropped);
    assert_eq!(
        harness.state().state.active_object().unwrap().id,
        original_tab.id
    );
    let page = harness.state().state.data.as_ref().unwrap();
    let columns = page
        .columns
        .iter()
        .map(|field| (field.name.clone(), field.native_number))
        .collect::<Vec<_>>();
    let survivors = page
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| (cell.text.clone(), cell.kind))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    // Each independent source drops its latest logical declaration; the retained
    // high-water mark therefore stays one above that retired field number.
    let number = usize::from(raw::identity(case).native_number) + 1;
    let mut values = vec![None; survivors.len()];
    let mut previous = readdition::after(&dropped, target);
    add_field(
        &mut harness,
        target.name(),
        match target {
            Target::Number => FieldAdditionKind::Number,
            Target::ShortText => FieldAdditionKind::ShortText,
        },
    );
    assert_eq!(
        std::fs::read(&path).unwrap(),
        previous,
        "Re-ADD {case:?} {target:?}"
    );
    assert_eq!(&previous[8 * 4096..9 * 4096], &dropped[8 * 4096..9 * 4096]);
    assert_rows(&harness, &columns, &survivors, &values, target, number);
    assert_eq!(
        harness.state().state.active_object().unwrap().id,
        original_tab.id
    );

    for row in 0..survivors.len() {
        let (expected, input) = match target {
            Target::Number => {
                let value = [91, 0, -12, 123][row];
                (
                    readdition::fill_number(case, &previous, row, value),
                    value.to_string(),
                )
            }
            Target::ShortText => {
                let value = ["", "compressed label", "日本語 🦀", "last row"][row];
                (
                    readdition::fill_text(case, &previous, row, value),
                    value.into(),
                )
            }
        };
        let old_tabs = harness.state().state.object_tabs();
        begin_replacement(&mut harness, row as u32, target.name(), true, Some(&input));
        review_and_save(&mut harness, &path, &previous, &expected);
        values[row] = Some(input);
        assert_rows(&harness, &columns, &survivors, &values, target, number);
        assert!(!journal_path(&path).exists());
        let fresh_tabs = harness.state().state.object_tabs();
        assert_eq!(fresh_tabs.len(), old_tabs.len());
        for (old, fresh) in old_tabs.iter().zip(&fresh_tabs) {
            assert_eq!(fresh.target.kind, old.target.kind);
            // Same-document saves retain presentation IDs and tab order. The
            // fresh document revision revokes every complete old tab target.
            assert_eq!(fresh.id, old.id);
            assert!(fresh.target.document_revision > old.target.document_revision);
            assert!(!harness.state_mut().state.activate_object(old));
        }
        previous = expected;
    }
    let current = harness.state().state.active_object().unwrap();
    assert_eq!(current.target.kind, original_tab.target.kind);
    assert!(current.target.document_revision > original_tab.target.document_revision);
    let scratch_tab = harness
        .state()
        .state
        .object_tabs()
        .into_iter()
        .find(|tab| matches!(tab.target.kind, ObjectKind::Sql))
        .unwrap();
    harness.state_mut().activate_object(scratch_tab);
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
    harness.state_mut().activate_object(current);
    finish_reads(&mut harness);
    assert_rows(&harness, &columns, &survivors, &values, target, number);
    drop(harness);
    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    assert_rows(&reopened, &columns, &survivors, &values, target, number);
    assert_eq!(std::fs::read(&path).unwrap(), previous);
    assert!(!journal_path(&path).exists());
}

#[test]
fn number_present_drop_then_add_number_and_fill_old_rows() {
    journey(raw::Case::NumberPresent, Target::Number);
}

#[test]
fn number_present_drop_then_add_text_and_fill_old_rows() {
    journey(raw::Case::NumberPresent, Target::ShortText);
}

#[test]
fn number_absent_drop_then_add_number_and_fill_old_rows() {
    journey(raw::Case::NumberAbsent, Target::Number);
}

#[test]
fn number_absent_drop_then_add_text_and_fill_old_rows() {
    journey(raw::Case::NumberAbsent, Target::ShortText);
}

#[test]
fn text_present_drop_then_add_number_and_fill_old_rows() {
    journey(raw::Case::TextPresent, Target::Number);
}

#[test]
fn text_present_drop_then_add_text_and_fill_old_rows() {
    journey(raw::Case::TextPresent, Target::ShortText);
}

#[test]
fn text_absent_drop_then_add_number_and_fill_old_rows() {
    journey(raw::Case::TextAbsent, Target::Number);
}

#[test]
fn text_absent_drop_then_add_text_and_fill_old_rows() {
    journey(raw::Case::TextAbsent, Target::ShortText);
}

#[test]
fn mixed_text_drop_then_add_number_and_fill_old_rows() {
    journey(raw::Case::MixedText, Target::Number);
}

#[test]
fn mixed_text_drop_then_add_text_and_fill_old_rows() {
    journey(raw::Case::MixedText, Target::ShortText);
}

#[test]
fn mixed_binary_drop_then_add_number_and_fill_old_rows() {
    journey(raw::Case::MixedBinary, Target::Number);
}

#[test]
fn mixed_binary_drop_then_add_text_and_fill_old_rows() {
    journey(raw::Case::MixedBinary, Target::ShortText);
}

#[test]
fn id_only_text_drop_then_add_number_and_fill_old_rows() {
    journey(raw::Case::IdOnlyText, Target::Number);
}

#[test]
fn id_only_text_drop_then_add_text_and_fill_old_rows() {
    journey(raw::Case::IdOnlyText, Target::ShortText);
}
