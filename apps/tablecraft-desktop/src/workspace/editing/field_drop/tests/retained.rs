//! Actual retained ADD/edit/RENAME/DROP/re-ADD cycles keep native history usable.
use super::{Entry, choose, inspect};
use crate::workspace::{
    Desktop,
    test_support::{
        app_harness,
        cell_edit::{begin_replacement, review_and_save},
        field_addition::add_field,
        field_rename::rename_and_save,
        finish_reads, finish_schema_save,
        inserting::{begin_generated_insert, choose_field_mode, review_and_save_insert},
        objects::{EXACT_SCRATCH, scratch},
        opened,
    },
};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_builders::table::FieldAdditionKind;
use tablecraft_core::values::Value;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{
    field_changes as changes,
    fields::{self, Case, Target},
    rows::Input,
};

fn open_table(harness: &mut Harness<'_, Desktop>) {
    harness
        .get_by_role_and_label(Role::Button, "Table1")
        .focus();
    harness.run_steps(1);
    harness.key_press(Key::Enter);
    harness.run_steps(1);
    finish_reads(harness);
}

fn journey(case: Case, target: Target) {
    let original = fields::source(case);
    let roots = original.roots.to_vec();
    let source = fields::property_pages::extend_source(
        original,
        fields::property_pages::Case::OriginalInsufficient,
    );
    let (_directory, path, mut previous, mut harness) = opened(source.bytes);
    open_table(&mut harness);
    let tab = harness.state().state.active_object().unwrap();
    scratch(&mut harness, &tab);
    let kind = match target {
        Target::Number => FieldAdditionKind::Number,
        Target::ShortText => FieldAdditionKind::ShortText,
    };
    let added = fields::after(&previous, source.definition, source.catalog_root, target);
    add_field(&mut harness, target.name(), kind);
    assert_eq!(std::fs::read(&path).unwrap(), added);
    let (first, text) = match target {
        Target::Number => (Value::Int32(123), "123"),
        Target::ShortText => (Value::Text("first".into()), "first"),
    };
    let filled = fields::fill(&added, source.definition, roots[0], target, Some(first));
    begin_replacement(&mut harness, 0, target.name(), true, Some(text));
    review_and_save(&mut harness, &path, &added, &filled);
    previous = filled;
    let mut old = target.name();
    for name in ["ExpandedRenamedColumn", "X", "x", "x", target.name()] {
        let after = changes::rename(&previous, source.definition, source.catalog_root, old, name);
        rename_and_save(&mut harness, &path, &previous, &after, old, name);
        previous = after;
        old = name;
    }
    let (second, text) = match target {
        Target::Number => (Value::Int32(-7), "-7"),
        Target::ShortText => (Value::Text("日本語🦀".into()), "日本語🦀"),
    };
    let edited = fields::fill(&previous, source.definition, roots[1], target, Some(second));
    begin_replacement(&mut harness, 1, old, true, Some(text));
    review_and_save(&mut harness, &path, &previous, &edited);
    let dropped = changes::drop_tail(&edited, source.definition, source.catalog_root, old);
    inspect(&mut harness, old, Entry::Datasheet);
    choose(&mut harness, "field_drop_confirmation", "Yes");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::S);
    harness.run_steps(1);
    finish_schema_save(&mut harness);
    assert_eq!(std::fs::read(&path).unwrap(), dropped);
    let readded = fields::after(&dropped, source.definition, source.catalog_root, target);
    add_field(&mut harness, target.name(), kind);
    assert_eq!(std::fs::read(&path).unwrap(), readded);
    assert!(
        harness
            .state()
            .state
            .data
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .all(|row| row.last().unwrap().text == "NULL")
    );
    let replacement = match target {
        Target::Number => Value::Int32(91),
        Target::ShortText => Value::Text("readded".into()),
    };
    let refilled = fields::fill(
        &readded,
        source.definition,
        roots[0],
        target,
        Some(replacement),
    );
    begin_replacement(
        &mut harness,
        0,
        target.name(),
        true,
        Some(match target {
            Target::Number => "91",
            Target::ShortText => "readded",
        }),
    );
    review_and_save(&mut harness, &path, &readded, &refilled);
    let expected = fields::append(&refilled, source.definition, target, Input::Omitted);
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
    begin_generated_insert(&mut harness);
    for column in columns {
        choose_field_mode(&mut harness, &column, "Omitted");
    }
    review_and_save_insert(&mut harness, &path, &refilled);
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
    drop(harness);
    let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut fresh);
    open_table(&mut fresh);
    let page = fresh.state().state.data.as_ref().unwrap();
    assert_eq!(page.rows.len(), roots.len() + 1);
    assert_eq!(page.columns.last().unwrap().name, target.name());
    assert_eq!(page.rows[1].last().unwrap().text, "NULL");
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(!journal_path(&path).exists());
}

#[test]
fn retained_number_schema_rename_drop_readd_and_insert_keep_other_table() {
    journey(Case::FirstNumber, Target::Number);
}

#[test]
fn retained_text_schema_long_roots_rename_drop_readd_and_insert_keep_history() {
    journey(Case::SecondText, Target::ShortText);
}

#[test]
fn original_sparse_schema_rename_drop_readd_and_insert_keep_appended_sibling() {
    journey(Case::OriginalSparse, Target::ShortText);
}
