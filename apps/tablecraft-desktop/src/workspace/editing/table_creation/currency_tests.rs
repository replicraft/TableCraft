//! Exact Currency input survives ordinary and retained reviewed saves.
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

fn open_table(harness: &mut Harness<'_, Desktop>, table: &str) {
    harness.get_by_role_and_label(Role::Button, table).focus();
    harness.run_steps(1);
    harness.key_press(Key::Enter);
    finish_reads(harness);
    assert_eq!(harness.state().state.data.as_ref().unwrap().table, table);
}

fn insert(
    harness: &mut Harness<'_, Desktop>,
    path: &Path,
    before: &[u8],
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
    let expected = raw::append(before, 7, input);
    assert_eq!(std::fs::read(path).unwrap(), expected);
    expected
}

#[test]
fn currency_precision_and_null_survive_created_table_then_retained_sibling_saves() {
    let (_directory, path, mut harness) = initial_table();
    add_named_field(&mut harness, "table-draft-currency", "Amount");
    start_save(&mut harness, "Table1");
    finish_reads(&mut harness);
    let mut expected = raw::source();
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    let table = harness.state().state.active_object().unwrap();
    scratch(&mut harness, &table);

    // Omission resolves the stored Currency default; precise typed input never
    // comes from the formatted two-decimal datasheet cell.
    expected = insert(&mut harness, &path, &expected, Input::Omitted, None);
    expected = insert(
        &mut harness,
        &path,
        &expected,
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
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(!journal_path(&path).exists());
}
