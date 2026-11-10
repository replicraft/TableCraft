//! Actual rename saves preserve sparse declaration identities and retained histories.
use crate::workspace::Desktop;
use crate::workspace::test_support::cell_edit::{begin_replacement, review_and_save};
use crate::workspace::editing::test_support::field_rename::propose_name;
use crate::workspace::test_support::objects::{EXACT_SCRATCH, scratch};
use crate::workspace::test_support::{app_harness, finish_reads, finish_schema_save, opened};
use egui::{Key, Modifiers};
use egui_kittest::Harness;
use std::time::{Duration, Instant};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::populated::{
        self as raw,
        readdition::{self, Target},
    },
    saved_field_rename::fitting::current_history,
};
use tablecraft_ui::{CellKind, ObjectKind};

fn assert_contents(
    harness: &Harness<'_, Desktop>,
    columns: &[(String, Option<usize>)],
    rows: &[Vec<(String, CellKind)>],
    selected: usize,
    name: &str,
) {
    let page = harness.state().state.data.as_ref().unwrap();
    assert_eq!(page.columns.len(), columns.len());
    for (index, (field, (original_name, number))) in page.columns.iter().zip(columns).enumerate() {
        assert_eq!(
            field.name,
            if index == selected {
                name
            } else {
                original_name
            }
        );
        assert_eq!(&field.native_number, number);
    }
    assert_eq!(page.rows.len(), rows.len());
    for (row, original) in page.rows.iter().zip(rows) {
        assert_eq!(row.len(), original.len());
        for (cell, (text, kind)) in row.iter().zip(original) {
            assert_eq!(&cell.text, text);
            assert!(cell.kind == *kind, "Rename changed a stored cell kind");
        }
    }
}

fn journey(case: raw::Case, target: Target, filled: bool) {
    let before = current_history::source(case, target, filled);
    let (_directory, path, mut previous, mut harness) = opened(before);
    let original_tab = harness.state().state.active_object().unwrap();
    scratch(&mut harness, &original_tab);
    let page = harness.state().state.data.as_ref().unwrap();
    let selected = page
        .columns
        .iter()
        .position(|field| field.name == target.name())
        .unwrap();
    let columns = page
        .columns
        .iter()
        .map(|field| (field.name.clone(), field.native_number))
        .collect::<Vec<_>>();
    let mut rows = page
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .map(|cell| (cell.text.clone(), cell.kind))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut old_name = target.name().to_string();
    for name in ["ALongerRenamedField", "X", "x", "x"] {
        let expected = current_history::rename(&previous, &old_name, name);
        let unchanged = old_name == name;
        let old_tabs = harness.state().state.object_tabs();
        propose_name(&mut harness, &old_name, name);
        assert_eq!(
            harness.state().state.field_rename_proposal().unwrap().1,
            name
        );
        assert!(harness.state().state.can_review_field_rename());
        assert_eq!(
            std::fs::read(&path).unwrap(),
            previous,
            "Name proposal is inert"
        );
        assert!(!journal_path(&path).exists());
        harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
        harness.run_steps(1);
        if unchanged {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                assert!(
                    harness.state().editing.save.is_none(),
                    "Unchanged review must not invent a save actor"
                );
                assert!(!harness.state().editing.worker.is_busy());
                if !harness.state().state.busy && !harness.state().editing.has_save() {
                    break;
                }
                assert!(
                    Instant::now() < deadline,
                    "Unchanged rename witness did not finish"
                );
                harness.run_steps(1);
                std::thread::yield_now();
            }
            finish_reads(&mut harness);
            assert_eq!(
                expected, previous,
                "No-op must not append a history snapshot"
            );
        } else {
            finish_schema_save(&mut harness);
        }
        assert_eq!(
            std::fs::read(&path).unwrap(),
            expected,
            "Rename {case:?} {target:?} filled={filled} to {name}"
        );
        assert_eq!(
            &expected[8 * 4096..10 * 4096],
            &previous[8 * 4096..10 * 4096]
        );
        assert!(!journal_path(&path).exists());
        assert!(!harness.state().state.field_rename_open());
        assert_contents(&harness, &columns, &rows, selected, name);
        let fresh_tabs = harness.state().state.object_tabs();
        assert_eq!(fresh_tabs.len(), old_tabs.len());
        for (old, fresh) in old_tabs.iter().zip(&fresh_tabs) {
            assert_eq!(fresh.id, old.id);
            assert_eq!(fresh.target.kind, old.target.kind);
            assert!(fresh.target.document_revision > old.target.document_revision);
        }
        previous = expected;
        old_name = name.into();
    }
    let current = harness.state().state.active_object().unwrap();
    assert_eq!(current.id, original_tab.id);
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
    assert_contents(&harness, &columns, &rows, selected, "x");
    if filled && matches!(case, raw::Case::MixedBinary) {
        // These two representatives also cross from retained metadata rename
        // back to ordinary cell editing, keeping opaque Binary siblings intact.
        let (expected, input) = match target {
            Target::Number => (readdition::fill_number(case, &previous, 1, -42), "-42"),
            Target::ShortText => (
                readdition::fill_text(case, &previous, 1, "日本語"),
                "日本語",
            ),
        };
        begin_replacement(&mut harness, 1, "x", true, Some(input));
        review_and_save(&mut harness, &path, &previous, &expected);
        rows[1][selected].0 = input.into();
        let CellKind::Scalar { is_null, .. } = &mut rows[1][selected].1 else {
            panic!("The independent target is a scalar Number or Short Text");
        };
        *is_null = false;
        previous = expected;
        assert_contents(&harness, &columns, &rows, selected, "x");
    }
    drop(harness);
    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    assert_contents(&reopened, &columns, &rows, selected, "x");
    assert_eq!(std::fs::read(&path).unwrap(), previous);
    assert!(!journal_path(&path).exists());
}

#[test]
fn number_present_filled_number_renames_grow_shrink_case_and_noop() {
    journey(raw::Case::NumberPresent, Target::Number, true);
}

#[test]
fn number_present_filled_text_renames_grow_shrink_case_and_noop() {
    journey(raw::Case::NumberPresent, Target::ShortText, true);
}

#[test]
fn number_absent_filled_number_renames_grow_shrink_case_and_noop() {
    journey(raw::Case::NumberAbsent, Target::Number, true);
}

#[test]
fn number_absent_filled_text_renames_grow_shrink_case_and_noop() {
    journey(raw::Case::NumberAbsent, Target::ShortText, true);
}

#[test]
fn text_present_filled_number_renames_grow_shrink_case_and_noop() {
    journey(raw::Case::TextPresent, Target::Number, true);
}

#[test]
fn text_present_filled_text_renames_grow_shrink_case_and_noop() {
    journey(raw::Case::TextPresent, Target::ShortText, true);
}

#[test]
fn text_absent_filled_number_renames_grow_shrink_case_and_noop() {
    journey(raw::Case::TextAbsent, Target::Number, true);
}

#[test]
fn text_absent_filled_text_renames_grow_shrink_case_and_noop() {
    journey(raw::Case::TextAbsent, Target::ShortText, true);
}

#[test]
fn mixed_text_filled_number_renames_grow_shrink_case_and_noop() {
    journey(raw::Case::MixedText, Target::Number, true);
}

#[test]
fn mixed_text_filled_text_renames_grow_shrink_case_and_noop() {
    journey(raw::Case::MixedText, Target::ShortText, true);
}

#[test]
fn mixed_binary_filled_number_renames_then_edits_the_current_field() {
    journey(raw::Case::MixedBinary, Target::Number, true);
}

#[test]
fn mixed_binary_filled_text_renames_then_edits_the_current_field() {
    journey(raw::Case::MixedBinary, Target::ShortText, true);
}

#[test]
fn id_only_text_filled_number_renames_grow_shrink_case_and_noop() {
    journey(raw::Case::IdOnlyText, Target::Number, true);
}

#[test]
fn id_only_text_filled_text_renames_grow_shrink_case_and_noop() {
    journey(raw::Case::IdOnlyText, Target::ShortText, true);
}

#[test]
fn unfilled_number_readdition_renames_without_materializing_old_rows() {
    journey(raw::Case::IdOnlyText, Target::Number, false);
}

#[test]
fn unfilled_text_readdition_renames_without_materializing_old_rows() {
    journey(raw::Case::IdOnlyText, Target::ShortText, false);
}
