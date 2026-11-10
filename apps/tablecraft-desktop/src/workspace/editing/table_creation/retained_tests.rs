//! Real DROP → Create → save journeys retain history and separate schema/row saves.
use super::{
    creating_tests::create_table,
    design_tests::view,
    properties_tests::replace,
    test_support::{
        add_named_field, apply_and_acknowledge, field_label, review, start_save,
        wait_for_bound_record,
    },
};
use crate::workspace::{
    Desktop,
    editing::table_drop::tests::{
        choose,
        current::{close_table, confirmation, delete_navigation},
    },
    test_support::{
        app_harness, finish_reads, finish_schema_save,
        objects::{EXACT_SCRATCH, scratch},
        opened,
    },
};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use std::path::Path;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::populated::{
        Case, readdition::Target as Field,
    },
    canonical_retained_add_table::{self as raw, Target},
    canonical_table_drop::current::{self as dropped, History},
};

fn draft(harness: &mut Harness<'_, Desktop>, target: Target) {
    create_table(harness);
    harness
        .get_by_role_and_label(Role::Tab, "Home")
        .click_accesskit();
    harness.run_steps(2);
    view(harness, "ribbon-table-design");
    // The generated ID stays protected and retains its actual uppercase name.
    view(harness, "ribbon-datasheet");
    match target {
        Target::GeneratedNumber => add_named_field(harness, "table-draft-number", "Score"),
        Target::GeneratedText => add_named_field(harness, "table-draft-short-text", "Name"),
        _ => panic!("generated UI draft only"),
    }
}

fn save_schema(
    harness: &mut Harness<'_, Desktop>,
    path: &Path,
    before: &[u8],
    name: &str,
    target: Target,
) -> Vec<u8> {
    assert_eq!(std::fs::read(path).unwrap(), before);
    assert!(!journal_path(path).exists());
    let expected = raw::append_with_uppercase_id(before, name, target);
    start_save(harness, name);
    finish_reads(harness);
    assert_eq!(std::fs::read(path).unwrap(), expected);
    assert!(!journal_path(path).exists());
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.has_unsaved_table_drafts());
    assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
    expected
}

fn delete_original(harness: &mut Harness<'_, Desktop>, count: usize) {
    let table = harness.state().state.active_object().unwrap();
    scratch(harness, &table);
    close_table(harness, &table);
    delete_navigation(harness);
    confirmation(harness, count);
    choose(harness, "Yes");
    finish_schema_save(harness);
    assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
}

fn journey(case: Case, history: History, target: Target, name: &str) {
    let source = dropped::source(case, Field::ShortText, true, history);
    let deleted = dropped::after(&source);
    let (_directory, path, _, mut harness) = opened(source.bytes);
    delete_original(&mut harness, source.roots.len());
    assert_eq!(std::fs::read(&path).unwrap(), deleted);
    assert!(harness.state().state.can_create_table());
    draft(&mut harness, target);
    let first = save_schema(&mut harness, &path, &deleted, name, target);
    let first_tab = harness.state().state.active_object().unwrap();
    assert!(harness.state().state.data.as_ref().unwrap().rows.is_empty());
    assert!(harness.state().state.can_create_table());

    let second_target = if matches!(target, Target::GeneratedNumber) {
        Target::GeneratedText
    } else {
        Target::GeneratedNumber
    };
    draft(&mut harness, second_target);
    let second = save_schema(&mut harness, &path, &first, "Second", second_target);
    let refreshed_first = harness
        .state()
        .state
        .object_tabs()
        .iter()
        .find(|tab| tab.id == first_tab.id)
        .unwrap()
        .clone();
    harness.state_mut().activate_object(refreshed_first);
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.data.as_ref().unwrap().table, name);
    assert!(harness.state().state.data.as_ref().unwrap().rows.is_empty());
    assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
    drop(harness);

    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    let database = reopened.state().state.database.as_ref().unwrap();
    let tables = database
        .tables
        .iter()
        .filter(|table| !table.system)
        .collect::<Vec<_>>();
    assert_eq!(tables.len(), 2);
    assert!(tables.iter().any(|table| table.name == name));
    assert!(tables.iter().any(|table| table.name == "Second"));
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
    assert_eq!(std::fs::read(&path).unwrap(), second);
    assert!(!journal_path(&path).exists());
}

#[test]
fn retained_number_replacement_reuses_only_the_dead_name_then_appends_another_table() {
    journey(
        Case::NumberPresent,
        History::Inserted,
        Target::GeneratedNumber,
        "Table1",
    );
}

#[test]
fn retained_text_replacement_preserves_forwarded_history_and_an_independent_second_table() {
    journey(
        Case::MixedBinary,
        History::Deleted,
        Target::GeneratedText,
        "Replacement",
    );
}

#[test]
fn retained_empty_replacement_and_second_table_keep_generated_history_retired() {
    journey(
        Case::IdOnlyText,
        History::Empty,
        Target::GeneratedNumber,
        "Fresh",
    );
}

#[test]
fn first_record_after_retained_creation_uses_the_real_review_and_save_route() {
    let source = dropped::source(Case::TextPresent, Field::ShortText, true, History::Fields);
    let deleted = dropped::after(&source);
    let (_directory, path, _, mut harness) = opened(source.bytes);
    delete_original(&mut harness, source.roots.len());
    draft(&mut harness, Target::GeneratedText);
    let label = field_label(&harness, "table-draft-record-field", "Name");
    replace(&mut harness, Role::TextInput, &label, "new 日本語🦀");
    let schema = save_schema(
        &mut harness,
        &path,
        &deleted,
        "Replacement",
        Target::GeneratedText,
    );
    wait_for_bound_record(&mut harness, "Replacement");
    review(&mut harness);
    assert!(harness.state().editing.insert_review.is_some());
    assert_eq!(std::fs::read(&path).unwrap(), schema);
    apply_and_acknowledge(&mut harness);
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.table, "Replacement");
    assert_eq!(data.rows.len(), 1);
    assert_eq!(data.rows[0][0].text, "1");
    assert_eq!(data.rows[0][1].text, "new 日本語🦀");
    let saved = std::fs::read(&path).unwrap();
    assert_eq!(&saved[7 * 4096..deleted.len()], &deleted[7 * 4096..]);
    assert!(!journal_path(&path).exists());
    drop(harness);
    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    let data = reopened.state().state.data.as_ref().unwrap();
    assert_eq!(data.table, "Replacement");
    assert_eq!(data.rows.len(), 1);
    assert_eq!(data.rows[0][0].text, "1");
    assert_eq!(data.rows[0][1].text, "new 日本語🦀");
    assert_eq!(std::fs::read(&path).unwrap(), saved);
}
