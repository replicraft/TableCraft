//! Actual mixed DDL/DML reviews keep long catalog/user chains usable across saves.
use super::{draft, save_schema};
use crate::workspace::{
    Desktop,
    editing::test_support::{review_row_delete, save_reviewed_row},
    test_support::{
        app_harness,
        cell_edit::{begin_replacement, review_and_save},
        finish_reads,
        inserting::{begin_generated_insert, choose_field_mode, review_and_save_insert},
        objects::{EXACT_SCRATCH, scratch},
        opened,
    },
};
use egui::{Key, accesskit::Role};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_runtime::journal_path;
use tablecraft_testkit::fixtures::access::canonical_retained_add_table::{Target, cross_operation};

fn table(harness: &mut Harness<'_, Desktop>) {
    harness
        .get_by_role_and_label(Role::Button, "Table1")
        .focus();
    harness.run_steps(1);
    harness.key_press(Key::Enter);
    harness.run_steps(1);
    finish_reads(harness);
    assert_eq!(harness.state().state.data.as_ref().unwrap().table, "Table1");
}

#[test]
fn long_roots_keep_ddl_and_dml_available_across_the_same_reviewed_session() {
    let stages = cross_operation::source();
    let (_directory, path, _, mut harness) = opened(stages.initial);
    let original = harness.state().state.active_object().unwrap();
    scratch(&mut harness, &original);
    let before = std::fs::read(&path).unwrap();
    draft(&mut harness, Target::GeneratedText);
    let appended = save_schema(&mut harness, &path, &before, "Other", Target::GeneratedText);
    assert_eq!(appended, stages.appended);
    table(&mut harness);

    begin_generated_insert(&mut harness);
    choose_field_mode(&mut harness, "Score", "Omitted");
    review_and_save_insert(&mut harness, &path, &appended);
    assert_eq!(std::fs::read(&path).unwrap(), stages.inserted);
    begin_replacement(&mut harness, 0, "Score", false, Some("123"));
    review_and_save(&mut harness, &path, &stages.inserted, &stages.updated);

    for (before, after) in [
        (&stages.updated, &stages.first_deleted),
        (&stages.first_deleted, &stages.empty),
    ] {
        review_row_delete(&mut harness, 0);
        assert_eq!(std::fs::read(&path).unwrap(), *before);
        save_reviewed_row(&mut harness, "Delete reviewed row");
        assert_eq!(std::fs::read(&path).unwrap(), *after);
        assert!(!journal_path(&path).exists());
    }
    assert!(harness.state().state.data.as_ref().unwrap().rows.is_empty());
    begin_generated_insert(&mut harness);
    choose_field_mode(&mut harness, "Score", "Omitted");
    review_and_save_insert(&mut harness, &path, &stages.empty);
    assert_eq!(std::fs::read(&path).unwrap(), stages.reinserted);
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().rows[0][0].text,
        "3"
    );

    draft(&mut harness, Target::GeneratedNumber);
    let final_image = save_schema(
        &mut harness,
        &path,
        &stages.reinserted,
        "More",
        Target::GeneratedNumber,
    );
    assert_eq!(final_image, stages.appended_again);
    assert_eq!(harness.state().state.sql, EXACT_SCRATCH);
    drop(harness);

    let mut fresh = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut fresh);
    table(&mut fresh);
    let page = fresh.state().state.data.as_ref().unwrap();
    assert_eq!(page.rows.len(), 1);
    assert_eq!(
        page.rows[0]
            .iter()
            .map(|cell| cell.text.as_str())
            .collect::<Vec<_>>(),
        ["3", "0"]
    );
    for sibling in ["Other", "More"] {
        fresh.get_by_role_and_label(Role::Button, sibling).focus();
        fresh.run_steps(1);
        fresh.key_press(Key::Enter);
        fresh.run_steps(1);
        finish_reads(&mut fresh);
        let page = fresh.state().state.data.as_ref().unwrap();
        assert_eq!(page.table, sibling);
        assert!(page.rows.is_empty());
    }
    assert_eq!(std::fs::read(&path).unwrap(), final_image);
    assert!(!journal_path(&path).exists());
}
