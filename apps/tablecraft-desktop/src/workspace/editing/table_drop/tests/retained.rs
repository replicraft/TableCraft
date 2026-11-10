//! Actual table deletion and recreation retain sibling and retired storage history.
use super::{
    choose,
    current::{close_table, confirmation, delete_navigation},
};
use crate::workspace::{
    Desktop,
    editing::table_creation::retained_tests::{draft, save_schema},
    test_support::{
        app_harness, finish_reads, finish_schema_save,
        inserting::{begin_generated_insert, choose_field_mode, review_and_save_insert},
        objects::{EXACT_SCRATCH, scratch},
        opened,
    },
};
use egui::{Key, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::{
    canonical_retained_add_table::{
        Target,
        rows::{self, Input, Profile},
    },
    canonical_table_drop::retained::{self as raw, Case},
};
use tablecraft_ui::ObjectKind;

fn open_table(harness: &mut Harness<'_, Desktop>, name: &str) {
    harness.get_by_role_and_label(Role::Button, name).focus();
    harness.run_steps(1);
    harness.key_press(Key::Enter);
    harness.run_steps(1);
    finish_reads(harness);
    assert_eq!(harness.state().state.data.as_ref().unwrap().table, name);
}

fn journey(case: Case, name: &str, target: Target, cancel: bool) {
    let source = raw::source(case);
    let expected_drop = raw::after(
        &source.bytes,
        source.definition,
        source.catalog_root,
        &source.roots,
    );
    let (_directory, path, before, mut harness) = opened(source.bytes);
    open_table(&mut harness, "Table1");
    let original = harness.state().state.active_object().unwrap();
    let scratch_tab = scratch(&mut harness, &original);
    // The second-table source initially opens Other. Table DROP's existing
    // binding requires all native object tabs closed, so close each real tab;
    // keep the unsaved SQL scratch open throughout the operation.
    let native_tabs = harness
        .state()
        .state
        .object_tabs()
        .into_iter()
        .filter(|tab| matches!(tab.target.kind, ObjectKind::Table { .. }))
        .collect::<Vec<_>>();
    for tab in native_tabs {
        close_table(&mut harness, &tab);
    }
    delete_navigation(&mut harness);
    assert!(
        harness.state().state.table_drop_open(),
        "Initial DROP: {:?}",
        harness.state().state.error
    );
    confirmation(&harness, source.roots.len());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(!journal_path(&path).exists());
    if cancel {
        choose(&mut harness, "No");
        finish_reads(&mut harness);
        assert!(!harness.state().state.table_drop_open());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert!(!harness.state().editing.has_save());
        delete_navigation(&mut harness);
        assert!(
            harness.state().state.table_drop_open(),
            "Repeated DROP: {:?}",
            harness.state().state.error
        );
        confirmation(&harness, source.roots.len());
    }
    choose(&mut harness, "Yes");
    finish_schema_save(&mut harness);
    assert_eq!(std::fs::read(&path).unwrap(), expected_drop);
    assert!(!journal_path(&path).exists());
    assert!(!harness.state().state.can_activate_object(&original));
    assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
    assert!(
        harness
            .state()
            .state
            .object_tabs()
            .iter()
            .any(|tab| tab.id == scratch_tab.id)
    );
    let users = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .tables
        .iter()
        .filter(|table| !table.system)
        .map(|table| table.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(users, ["Other"]);

    let definition = (expected_drop.len() / 4096) as u32;
    draft(&mut harness, target);
    let recreated = save_schema(&mut harness, &path, &expected_drop, name, target);
    begin_generated_insert(&mut harness);
    let (profile, input, field) = if matches!(target, Target::GeneratedText) {
        choose_field_mode(&mut harness, "Name", "Value");
        harness
            .get_by_role_and_label(Role::TextInput, "Name")
            .focus();
        harness.run_steps(1);
        harness
            .get_by_role_and_label(Role::TextInput, "Name")
            .type_text("new 日本語🦀");
        harness.run_steps(2);
        (
            Profile::DesktopText,
            Input::Text("new 日本語🦀"),
            "new 日本語🦀",
        )
    } else {
        choose_field_mode(&mut harness, "Score", "Omitted");
        (Profile::DesktopNumber, Input::Omitted, "0")
    };
    let expected_insert = rows::append(&recreated, definition, profile, input);
    review_and_save_insert(&mut harness, &path, &recreated);
    assert_eq!(std::fs::read(&path).unwrap(), expected_insert);
    assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
    drop(harness);

    let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut fresh);
    open_table(&mut fresh, name);
    let page = fresh.state().state.data.as_ref().unwrap();
    assert_eq!(page.rows.len(), 1);
    assert_eq!(
        page.rows[0]
            .iter()
            .map(|cell| cell.text.as_str())
            .collect::<Vec<_>>(),
        ["1", field]
    );
    open_table(&mut fresh, "Other");
    let other = fresh.state().state.data.as_ref().unwrap();
    if matches!(case, Case::SecondText) {
        assert_eq!(other.rows.len(), 1);
        assert_eq!(
            other.rows[0]
                .iter()
                .map(|cell| cell.text.as_str())
                .collect::<Vec<_>>(),
            ["1", "99"]
        );
    } else {
        assert!(other.rows.is_empty());
    }
    assert_eq!(std::fs::read(path).unwrap(), expected_insert);
}

#[test]
fn populated_retained_number_drop_recreates_same_name_and_inserts_new_id() {
    journey(Case::FirstNumber, "Table1", Target::GeneratedNumber, false);
}

#[test]
fn long_catalog_and_user_text_drop_cancel_then_recreate_and_insert_unicode() {
    journey(Case::SecondText, "Table1", Target::GeneratedText, true);
}

#[test]
fn original_sparse_table_drop_keeps_live_sibling_and_creates_replacement() {
    journey(
        Case::OriginalSparse,
        "Replacement",
        Target::GeneratedNumber,
        false,
    );
}
