//! Real successive DROP saves leave ID-only tables editable and insertable.
use super::{Entry, choose, inspect};
use crate::workspace::{
    Desktop,
    test_support::{
        app_harness,
        cell_edit::{begin_replacement, review_and_save},
        field_addition::add_field,
        finish_reads, finish_schema_save,
        inserting::{begin_generated_insert, choose_field_mode, review_and_save_insert},
        objects::{EXACT_SCRATCH, scratch},
        opened,
    },
};
use egui::{Key, Modifiers};
use tablecraft_builders::table::FieldAdditionKind;
use tablecraft_core::values::Value;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes,
    fields::{self, Case, Target},
    rows::Input,
};

#[test]
fn retained_id_only_after_all_nonkey_drops_can_add_fill_and_insert_again() {
    let source = fields::source(Case::FirstNumber);
    let number = fields::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        Target::Number,
    );
    let before = fields::after(
        &number,
        source.definition,
        source.catalog_root,
        Target::ShortText,
    );
    let (_directory, path, mut previous, mut harness) = opened(before);
    let original = harness.state().state.active_object().unwrap();
    scratch(&mut harness, &original);

    // Remove three actual active tails, including the final original Score.
    // Native logical/variable high-water marks and historical records survive.
    for column in ["NextText", "NextNumber", "Score"] {
        let expected =
            field_changes::drop_tail(&previous, source.definition, source.catalog_root, column);
        inspect(&mut harness, column, Entry::Datasheet);
        choose(&mut harness, "field_drop_confirmation", "Yes");
        assert_eq!(std::fs::read(&path).unwrap(), previous);
        harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
        harness.run_steps(1);
        finish_schema_save(&mut harness);
        assert_eq!(std::fs::read(&path).unwrap(), expected);
        assert!(!journal_path(&path).exists());
        previous = expected;
    }
    let page = harness.state().state.data.as_ref().unwrap();
    assert_eq!(page.columns.len(), 1);
    assert_eq!(page.columns[0].native_number, Some(0));
    assert_eq!(page.rows.len(), 2);
    assert_eq!(page.rows[0][0].text, "1");
    assert_eq!(page.rows[1][0].text, "2");

    let added = fields::after(
        &previous,
        source.definition,
        source.catalog_root,
        Target::Number,
    );
    add_field(&mut harness, "NextNumber", FieldAdditionKind::Number);
    assert_eq!(std::fs::read(&path).unwrap(), added);
    let page = harness.state().state.data.as_ref().unwrap();
    assert_eq!(page.columns[1].native_number, Some(4));
    assert!(page.rows.iter().all(|row| row[1].text == "NULL"));

    let filled = fields::fill(
        &added,
        source.definition,
        source.roots[0],
        Target::Number,
        Some(Value::Int32(123)),
    );
    begin_replacement(&mut harness, 0, "NextNumber", true, Some("123"));
    review_and_save(&mut harness, &path, &added, &filled);

    let expected = fields::append(&filled, source.definition, Target::Number, Input::Omitted);
    begin_generated_insert(&mut harness);
    choose_field_mode(&mut harness, "NextNumber", "Omitted");
    review_and_save_insert(&mut harness, &path, &filled);
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
    assert!(!journal_path(&path).exists());
    drop(harness);

    let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut fresh);
    let page = fresh.state().state.data.as_ref().unwrap();
    assert_eq!(page.table, "Table1");
    assert_eq!(page.columns.len(), 2);
    assert_eq!(page.columns[1].name, "NextNumber");
    assert_eq!(page.columns[1].native_number, Some(4));
    assert_eq!(
        page.rows
            .iter()
            .map(|row| row
                .iter()
                .map(|cell| cell.text.as_str())
                .collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        [vec!["1", "123"], vec!["2", "NULL"], vec!["3", "0"]],
    );
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(!journal_path(&path).exists());
}
