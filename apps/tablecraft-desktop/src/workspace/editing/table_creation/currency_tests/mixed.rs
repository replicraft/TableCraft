//! Different column types retain their own update language within one table.
use super::{assert_image, open_table};
use crate::workspace::{
    Desktop,
    editing::table_creation::{
        creating_tests::create_table,
        test_support::{add_named_field, start_save},
    },
    test_support::{
        app_harness,
        cell_edit::{begin_replacement, review_and_save},
        finish_reads,
        inserting::{begin_generated_insert, choose_field_mode, review_and_save_insert},
        objects::{EXACT_SCRATCH, scratch},
        opened,
    },
};
use tablecraft_display::ScalarPresentation;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    currency::mixed as raw,
    fields::{self, Case},
};

#[test]
fn appended_mixed_table_dispatches_currency_number_and_text_edits_independently() {
    let source = fields::source(Case::FirstNumber).bytes;
    let definition = u32::try_from(source.len() / 4096).unwrap();
    let (_directory, path, _, mut harness) = opened(source.clone());
    let original_tab = harness.state().state.active_object().unwrap();
    scratch(&mut harness, &original_tab);
    create_table(&mut harness);
    for (key, name) in [
        ("table-draft-number", "Score"),
        ("table-draft-currency", "Amount"),
        ("table-draft-short-text", "Label"),
    ] {
        add_named_field(&mut harness, key, name);
    }
    start_save(&mut harness, "MixedValues");
    finish_reads(&mut harness);
    let mut expected = raw::append_table(&source, "MixedValues");
    assert_image(&path, &expected);

    begin_generated_insert(&mut harness);
    for field in ["Score", "Amount", "Label"] {
        choose_field_mode(&mut harness, field, "Omitted");
    }
    review_and_save_insert(&mut harness, &path, &expected);
    expected = raw::append(&expected, definition);
    assert_image(&path, &expected);
    let row = &harness.state().state.data.as_ref().unwrap().rows[0];
    assert_eq!(row[0].text, "1");
    assert_eq!(row[1].text, "0");
    assert!(matches!(
        row[2].presentation,
        Some(ScalarPresentation::Currency { scaled: 0, .. })
    ));
    assert!(matches!(
        row[3].kind,
        tablecraft_ui::CellKind::Scalar { is_null: true, .. }
    ));

    begin_replacement(&mut harness, 0, "Amount", false, Some("1.2345"));
    let after = raw::update_currency(&expected, definition, 0, Some(12_345));
    review_and_save(&mut harness, &path, &expected, &after);
    expected = after;

    begin_replacement(&mut harness, 0, "Score", false, Some("41"));
    let after = raw::update_number(&expected, definition, 0, Some(41));
    review_and_save(&mut harness, &path, &expected, &after);
    expected = after;

    let label = "Mixed🦀".repeat(30);
    begin_replacement(&mut harness, 0, "Label", true, Some(&label));
    let after = raw::update_text(&expected, definition, 0, Some(&label));
    review_and_save(&mut harness, &path, &expected, &after);
    expected = after;
    assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
    drop(harness);

    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    open_table(&mut reopened, "MixedValues");
    let rows = &reopened.state().state.data.as_ref().unwrap().rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0][0].text, "1");
    assert_eq!(rows[0][1].text, "41");
    assert!(matches!(
        rows[0][2].presentation,
        Some(ScalarPresentation::Currency { scaled: 12_345, .. })
    ));
    assert_eq!(rows[0][3].text, label);
    open_table(&mut reopened, "Table1");
    assert_eq!(reopened.state().state.data.as_ref().unwrap().rows.len(), 2);
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
