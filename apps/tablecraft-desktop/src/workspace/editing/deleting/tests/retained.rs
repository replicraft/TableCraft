//! Reviewed retained-table DELETE preserves physical history through reinsertion.
use crate::workspace::{
    Desktop,
    editing::test_support::{review_row_delete, save_reviewed_row},
    test_support::{
        app_harness, finish_reads,
        inserting::{begin_generated_insert, choose_field_mode, review_and_save_insert},
        opened,
    },
};
use egui::{Key, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use std::cell::RefCell;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::{
        current_delete, current_insert,
        deleted_field::populated::{Case, readdition::Target as Field},
    },
    canonical_retained_add_table::{
        self as raw, Target, deletes,
        rows::{self, Input, Profile},
        updates,
    },
    canonical_table_drop::current as dropped,
};

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

fn texts(harness: &Harness<'_, Desktop>) -> Vec<Vec<String>> {
    harness
        .state()
        .state
        .data
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .map(|row| row.iter().map(|cell| cell.text.clone()).collect())
        .collect()
}

fn journey(
    bytes: Vec<u8>,
    roots: &[usize],
    next_id: i32,
    delete: impl Fn(&[u8], usize) -> Vec<u8>,
    insert: impl Fn(&[u8]) -> Vec<u8>,
) {
    let (_directory, path, mut previous, mut harness) = opened(bytes);
    open_table(&mut harness);
    let mut expected_rows = texts(&harness);
    assert_eq!(expected_rows.len(), roots.len());
    let columns = harness
        .state()
        .state
        .data
        .as_ref()
        .unwrap()
        .columns
        .iter()
        .skip(1)
        .map(|field| field.name.clone())
        .collect::<Vec<_>>();

    review_row_delete(&mut harness, 0);
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click_accesskit();
    harness.run_steps(2);
    assert!(harness.state().editing.delete_review.is_none());
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), previous);
    assert_eq!(texts(&harness), expected_rows);
    assert!(!journal_path(&path).exists());

    for root in roots {
        let expected = delete(&previous, *root);
        review_row_delete(&mut harness, 0);
        assert_eq!(std::fs::read(&path).unwrap(), previous, "Review is inert");
        save_reviewed_row(&mut harness, "Delete reviewed row");
        assert_eq!(std::fs::read(&path).unwrap(), expected);
        expected_rows.remove(0);
        assert_eq!(texts(&harness), expected_rows);
        assert!(!journal_path(&path).exists());
        previous = expected;
    }
    assert!(harness.state().state.current_delete_target().is_none());

    let expected = insert(&previous);
    begin_generated_insert(&mut harness);
    for column in columns {
        choose_field_mode(&mut harness, &column, "Omitted");
    }
    review_and_save_insert(&mut harness, &path, &previous);
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    let final_rows = texts(&harness);
    assert_eq!(final_rows.len(), 1);
    assert_eq!(final_rows[0][0], next_id.to_string());
    drop(harness);

    let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut fresh);
    open_table(&mut fresh);
    assert_eq!(texts(&fresh), final_rows);
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(!journal_path(&path).exists());
}

#[test]
fn retained_number_deletes_to_empty_then_inserts_after_issued_ids() {
    let old = dropped::source(
        Case::NumberPresent,
        Field::ShortText,
        true,
        dropped::History::Inserted,
    );
    let mut bytes = raw::desktop::append(&dropped::after(&old), "Table1", Target::GeneratedNumber);
    for input in [Input::Number(17), Input::Null, Input::Number(-7)] {
        bytes = rows::append(&bytes, 11, Profile::DesktopNumber, input);
    }
    bytes = updates::number(&bytes, 11, 0, None);
    bytes = raw::append(&bytes, "Other", Target::GeneratedText);
    journey(
        bytes,
        &[0, 1, 2],
        4,
        |before, root| deletes::after(before, 11, &[root]),
        |before| rows::append(before, 11, Profile::DesktopNumber, Input::Omitted),
    );
}

#[test]
fn second_retained_text_table_deletes_forwarded_and_opaque_null_rows_then_reinserts() {
    let old = dropped::source(
        Case::MixedBinary,
        Field::ShortText,
        true,
        dropped::History::Deleted,
    );
    let mut bytes = raw::append(&dropped::after(&old), "Other", Target::GeneratedNumber);
    bytes = rows::append(&bytes, 11, Profile::NativeNumber, Input::Number(99));
    bytes = raw::desktop::append(&bytes, "Table1", Target::GeneratedText);
    for input in [Input::Text("first"), Input::Text(""), Input::Text("opaque")] {
        bytes = rows::append(&bytes, 14, Profile::DesktopText, input);
    }
    bytes = updates::text(&bytes, 14, 0, Some("forwarded replacement 日本語🦀"));
    bytes = updates::with_opaque_text_null(&bytes, 14, 2);
    journey(
        bytes,
        &[0, 1, 2],
        4,
        |before, root| deletes::after(before, 14, &[root]),
        |before| rows::append(before, 14, Profile::DesktopText, Input::Omitted),
    );
}

#[test]
fn original_sparse_table_deletion_with_live_appended_sibling_keeps_generation_history() {
    let source = current_delete::source(Case::MixedBinary, Field::ShortText, true, true, true);
    let roots = source.roots;
    let next_id = source.next_id;
    let original = RefCell::new(source.bytes);
    let bytes = raw::append(&original.borrow(), "More", Target::GeneratedNumber);
    journey(
        bytes,
        &roots,
        next_id,
        |_before, root| {
            let old = current_delete::after(&original.borrow(), &[root]);
            let expected = raw::append(&old, "More", Target::GeneratedNumber);
            *original.borrow_mut() = old;
            expected
        },
        |_before| {
            let old = current_insert::append(
                &original.borrow(),
                Case::MixedBinary,
                Field::ShortText,
                current_insert::Input::Omitted,
            );
            raw::append(&old, "More", Target::GeneratedNumber)
        },
    );
}
