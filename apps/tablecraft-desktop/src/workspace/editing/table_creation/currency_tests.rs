//! Exact Currency input survives real schema and separately reviewed row saves.
mod mixed;
use super::{
    retained_tests,
    test_support::{add_named_field, initial_table, start_save},
};
use crate::workspace::{
    Desktop,
    test_support::{
        app_harness,
        cell_edit::{begin_replacement, review_and_save},
        finish_reads,
        inserting::{begin_generated_insert, choose_field_mode, review_and_save_insert},
        objects::{EXACT_SCRATCH, scratch},
    },
};
use egui::{Key, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use std::path::Path;
use tablecraft_display::ScalarPresentation;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    Target,
    currency::{self as raw, Input},
};

fn assert_image(path: &Path, expected: &[u8]) {
    let actual = std::fs::read(path).unwrap();
    assert_eq!(actual.len(), expected.len(), "complete image length");
    let difference = actual
        .iter()
        .zip(expected)
        .enumerate()
        .find(|(_, (actual, expected))| actual != expected);
    assert!(
        difference.is_none(),
        "first image byte mismatch: {difference:?}"
    );
}

fn open_table(harness: &mut Harness<'_, Desktop>, table: &str) {
    harness.get_by_role_and_label(Role::Button, table).focus();
    harness.run_steps(1);
    harness.key_press(Key::Enter);
    harness.run_steps(1);
    finish_reads(harness);
    assert_eq!(harness.state().state.data.as_ref().unwrap().table, table);
}

fn insert(
    harness: &mut Harness<'_, Desktop>,
    path: &Path,
    before: &[u8],
    definition: u32,
    input: Input,
    text: Option<&str>,
) -> Vec<u8> {
    begin_generated_insert(harness);
    if let Some(text) = text {
        choose_field_mode(harness, "Amount", "Value");
        harness
            .get_by_role_and_label(Role::TextInput, "Amount")
            .focus();
        harness.run_steps(1);
        harness
            .get_by_role_and_label(Role::TextInput, "Amount")
            .type_text(text);
        harness.run_steps(2);
    } else {
        choose_field_mode(harness, "Amount", "Omitted");
    }
    review_and_save_insert(harness, path, before);
    let expected = raw::append(before, definition, input);
    assert_image(path, &expected);
    expected
}

#[test]
fn currency_precision_and_null_survive_created_table_then_appended_sibling_saves() {
    let (_directory, path, mut harness) = initial_table();
    add_named_field(&mut harness, "table-draft-currency", "Amount");
    start_save(&mut harness, "Table1");
    finish_reads(&mut harness);
    let mut expected = raw::source();
    assert_image(&path, &expected);
    let table = harness.state().state.active_object().unwrap();
    scratch(&mut harness, &table);

    // Omission resolves the stored Currency default; precise typed input never
    // comes from the formatted two-decimal datasheet cell.
    expected = insert(&mut harness, &path, &expected, 7, Input::Omitted, None);
    expected = insert(
        &mut harness,
        &path,
        &expected,
        7,
        Input::Value(12_345),
        Some("1.2345"),
    );
    begin_replacement(&mut harness, 1, "Amount", false, Some("2.3456"));
    let edited = raw::update(&expected, 7, 1, Some(23_456));
    review_and_save(&mut harness, &path, &expected, &edited);
    expected = edited;

    retained_tests::draft(&mut harness, Target::GeneratedNumber);
    expected = retained_tests::save_schema(
        &mut harness,
        &path,
        &expected,
        "Other",
        Target::GeneratedNumber,
    );
    open_table(&mut harness, "Table1");
    for (was_null, value, text) in [
        (false, Some(-34_567), Some("-3.4567")),
        (false, None, None),
        (true, Some(45_678), Some("4.5678")),
    ] {
        begin_replacement(&mut harness, 1, "Amount", was_null, text);
        let edited = raw::update(&expected, 7, 1, value);
        review_and_save(&mut harness, &path, &expected, &edited);
        expected = edited;
        assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
    }
    assert!(!journal_path(&path).exists());
    drop(harness);

    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    open_table(&mut reopened, "Table1");
    let rows = &reopened.state().state.data.as_ref().unwrap().rows;
    assert_eq!(rows.len(), 2);
    for (row, (id, scaled)) in rows.iter().zip([("1", 0), ("2", 45_678)]) {
        assert_eq!(row[0].text, id);
        assert!(matches!(
            row[1].presentation,
            Some(ScalarPresentation::Currency { scaled: actual, .. }) if actual == scaled
        ));
    }
    open_table(&mut reopened, "Other");
    assert!(
        reopened
            .state()
            .state
            .data
            .as_ref()
            .unwrap()
            .rows
            .is_empty()
    );
    assert_image(&path, &expected);
    assert!(!journal_path(&path).exists());
}

#[test]
fn appended_currency_table_preserves_precise_values_null_and_original_siblings() {
    use super::creating_tests::create_table;
    use crate::workspace::test_support::opened;
    use tablecraft_testkit::fixtures::access::canonical_retained_add_table::fields;

    let source = fields::source(fields::Case::FirstNumber).bytes;
    let definition = u32::try_from(source.len() / 4096).unwrap();
    let (_directory, path, _, mut harness) = opened(source.clone());
    open_table(&mut harness, "Table1");
    let original_rows = harness
        .state()
        .state
        .data
        .as_ref()
        .unwrap()
        .rows
        .iter()
        .map(|row| row.iter().map(|cell| cell.text.clone()).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let original_tab = harness.state().state.active_object().unwrap();
    scratch(&mut harness, &original_tab);

    create_table(&mut harness);
    add_named_field(&mut harness, "table-draft-currency", "Amount");
    start_save(&mut harness, "MoneyTable");
    finish_reads(&mut harness);
    let mut expected = raw::append_table(&source, "MoneyTable");
    assert_image(&path, &expected);
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().table,
        "MoneyTable"
    );
    assert_eq!(harness.state().state.sql, EXACT_SCRATCH);

    expected = insert(
        &mut harness,
        &path,
        &expected,
        definition,
        Input::Omitted,
        None,
    );
    expected = insert(
        &mut harness,
        &path,
        &expected,
        definition,
        Input::Value(12_345),
        Some("1.2345"),
    );
    for (was_null, value, text) in [
        (false, Some(-23_456), Some("-2.3456")),
        (false, None, None),
        (true, Some(34_567), Some("3.4567")),
    ] {
        begin_replacement(&mut harness, 1, "Amount", was_null, text);
        let edited = raw::update(&expected, definition, 1, value);
        review_and_save(&mut harness, &path, &expected, &edited);
        expected = edited;
        assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
    }
    drop(harness);

    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    open_table(&mut reopened, "MoneyTable");
    let rows = &reopened.state().state.data.as_ref().unwrap().rows;
    assert_eq!(rows.len(), 2);
    for (row, (id, scaled)) in rows.iter().zip([("1", 0), ("2", 34_567)]) {
        assert_eq!(row[0].text, id);
        assert!(matches!(
            row[1].presentation,
            Some(ScalarPresentation::Currency { scaled: actual, .. }) if actual == scaled
        ));
    }
    open_table(&mut reopened, "Table1");
    let rows = &reopened.state().state.data.as_ref().unwrap().rows;
    assert_eq!(rows.len(), original_rows.len());
    for (row, original) in rows.iter().zip(&original_rows) {
        for (cell, text) in row.iter().zip(original) {
            assert_eq!(&cell.text, text);
        }
    }
    open_table(&mut reopened, "Other");
    assert!(
        reopened
            .state()
            .state
            .data
            .as_ref()
            .unwrap()
            .rows
            .is_empty()
    );
    assert_image(&path, &expected);
    assert!(!journal_path(&path).exists());
}
