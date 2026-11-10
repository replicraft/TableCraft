//! Actual navigation deletion preserves retained schema/row histories and scratch tabs.
use super::choose;
use crate::workspace::{
    Desktop,
    test_support::objects::{EXACT_SCRATCH, scratch},
    test_support::{app_harness, finish_reads, finish_schema_save, opened},
};
use egui::{Key, accesskit::Role};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::sync::Arc;
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::{
    canonical_created_number::mixed::after_off::deleted_field::populated::{
        Case, readdition::Target,
    },
    canonical_table_drop::current::{self as raw, History},
};
use tablecraft_ui::{FluentArgs, ObjectTab, TableDropRequest};

fn delete_navigation(harness: &mut Harness<'_, Desktop>) {
    harness
        .get_by_role_and_label(Role::Button, "Table1")
        .focus();
    harness.run_steps(1);
    harness.key_press(Key::Delete);
    harness.run_steps(1);
    finish_reads(harness);
}

fn close_table(harness: &mut Harness<'_, Desktop>, table: &ObjectTab) {
    let tabs = harness.state().state.object_tabs();
    let position = tabs.iter().position(|tab| tab.id == table.id).unwrap();
    harness
        .get_all_by_role_and_label(Role::Button, "Close object")
        .nth(position)
        .unwrap()
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(harness);
    assert!(
        !harness
            .state()
            .state
            .object_tabs()
            .iter()
            .any(|tab| tab.id == table.id)
    );
}

fn confirmation(harness: &Harness<'_, Desktop>, count: usize) {
    assert!(harness.state().state.table_drop_open());
    let mut arguments = FluentArgs::new();
    arguments.set("table", "Table1");
    let text = harness
        .state()
        .state
        .localizer
        .format("table-drop-confirm", &arguments);
    harness.get_by_label(&text);
    let mut arguments = FluentArgs::new();
    arguments.set("count", count.to_string());
    let text = harness
        .state()
        .state
        .localizer
        .format("table-drop-records", &arguments);
    harness.get_by_label(&text);
    assert!(
        !harness
            .get_by_role_and_label(Role::Button, "Yes")
            .accesskit_node()
            .is_disabled()
    );
}

fn journey(case: Case, target: Target, history: History) {
    let source = raw::source(case, target, true, history);
    let expected = raw::after(&source);
    let (_directory, path, before, mut harness) = opened(source.bytes);
    let table = harness.state().state.active_object().unwrap();
    let revision = table.target.document_revision;
    let scratch_tab = scratch(&mut harness, &table);
    close_table(&mut harness, &table);
    assert_eq!(harness.state().state.object_tabs(), [scratch_tab.clone()]);
    delete_navigation(&mut harness);
    confirmation(&harness, source.roots.len());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!harness.state().editing.has_save());
    assert!(!journal_path(&path).exists());

    choose(&mut harness, "Yes");
    finish_schema_save(&mut harness);
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(!journal_path(&path).exists());
    assert!(!harness.state().state.table_drop_open());
    assert!(harness.state().editing.table_drop.is_none());
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().editing.worker.is_busy());
    let database = harness.state().state.database.as_ref().unwrap();
    assert!(database.document_revision > revision);
    assert!(database.tables.iter().all(|table| table.system));
    let tabs = harness.state().state.object_tabs();
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0].id, scratch_tab.id);
    assert_eq!(tabs[0].target.kind, scratch_tab.target.kind);
    assert!(tabs[0].target.document_revision > scratch_tab.target.document_revision);
    assert!(!harness.state_mut().state.activate_object(&table));
    assert_eq!(harness.state().state.sql, EXACT_SCRATCH);

    drop(harness);
    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    assert!(
        reopened
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .tables
            .iter()
            .all(|table| table.system)
    );
    assert!(reopened.state().state.object_tabs().is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(!journal_path(&path).exists());
}

fn cancelled(case: Case, target: Target) {
    let source = raw::source(case, target, true, History::Inserted);
    let (_directory, path, before, mut harness) = opened(source.bytes);
    let table = harness.state().state.active_object().unwrap();
    let original_rows = harness.state().state.data.as_ref().unwrap().rows.clone();
    let scratch_tab = scratch(&mut harness, &table);
    delete_navigation(&mut harness);
    assert!(!harness.state().state.table_drop_open());
    assert!(!harness.state().state.deletion_dialog_open());
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), before);

    close_table(&mut harness, &table);
    delete_navigation(&mut harness);
    confirmation(&harness, source.roots.len());
    let request = harness.state().state.table_drop_request().unwrap();
    let stale = Arc::new(TableDropRequest {
        id: request.id,
        target: request.target.clone(),
    });
    choose(&mut harness, "No");
    finish_reads(&mut harness);
    assert!(!harness.state().state.table_drop_open());
    assert_eq!(harness.state().state.object_tabs(), [scratch_tab]);
    harness.state_mut().save_table_drop(stale);
    finish_reads(&mut harness);
    assert!(harness.state().editing.table_drop.is_none());
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().editing.worker.is_busy());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!journal_path(&path).exists());
    assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
    drop(harness);

    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    let rows = &reopened.state().state.data.as_ref().unwrap().rows;
    assert_eq!(rows.len(), original_rows.len());
    for (actual, original) in rows.iter().zip(&original_rows) {
        for (actual, original) in actual.iter().zip(original) {
            assert_eq!(actual.text, original.text);
            assert_eq!(actual.kind, original.kind);
        }
    }
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn initial_number_table_drops_from_navigation() {
    journey(Case::NumberPresent, Target::Number, History::Initial);
}

#[test]
fn initial_text_table_drops_from_navigation() {
    journey(Case::TextPresent, Target::ShortText, History::Initial);
}

#[test]
fn fields_number_table_drops_from_navigation() {
    journey(Case::NumberPresent, Target::Number, History::Fields);
}

#[test]
fn fields_text_table_drops_from_navigation() {
    journey(Case::MixedBinary, Target::ShortText, History::Fields);
}

#[test]
fn inserted_number_table_drops_from_navigation() {
    journey(Case::NumberPresent, Target::Number, History::Inserted);
}

#[test]
fn inserted_text_table_drops_from_navigation() {
    journey(Case::MixedBinary, Target::ShortText, History::Inserted);
}

#[test]
fn deleted_number_table_drops_from_navigation() {
    journey(Case::NumberPresent, Target::Number, History::Deleted);
}

#[test]
fn deleted_text_table_drops_from_navigation() {
    journey(Case::MixedBinary, Target::ShortText, History::Deleted);
}

#[test]
fn empty_number_table_drops_from_navigation() {
    journey(Case::NumberPresent, Target::Number, History::Empty);
}

#[test]
fn empty_text_table_drops_from_navigation() {
    journey(Case::MixedBinary, Target::ShortText, History::Empty);
}

#[test]
fn number_open_object_cancel_and_stale_request_never_save() {
    cancelled(Case::NumberPresent, Target::Number);
}

#[test]
fn text_open_object_cancel_and_stale_request_never_save() {
    cancelled(Case::MixedBinary, Target::ShortText);
}
