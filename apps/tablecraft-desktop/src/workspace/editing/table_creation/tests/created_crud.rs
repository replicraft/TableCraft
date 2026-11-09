//! One ordinary created database crosses all row-edit owners and a fresh host.
use super::{created, insert_generated_row, start_save};
use crate::workspace::Desktop;
use crate::workspace::test_support::{app_harness, finish_reads, finish_save};
use egui::{Key, Modifiers, accesskit::Role};
use egui_kittest::kittest::Queryable;
use tablecraft_builders::table::DraftFieldType;
use tablecraft_runtime::{SaveOutcome, journal_path};

#[test]
fn created_number_table_round_trips_insert_change_delete_and_automatic_reopen() {
    let (_directory, path, mut harness) = created();
    assert!(
        harness
            .state_mut()
            .state
            .add_table_draft_field(DraftFieldType::Number)
    );
    harness.run_steps(2);
    start_save(&mut harness, "Contacts");
    finish_reads(&mut harness);
    insert_generated_row(&mut harness, Some("7"));
    insert_generated_row(&mut harness, Some("8"));
    let inserted = std::fs::read(&path).unwrap();
    assert!(!journal_path(&path).exists());
    drop(harness);

    // Ordinary file Open, rather than the save's refresh, must select Contacts.
    let mut harness = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut harness);
    let data = harness.state().state.data.as_ref().unwrap();
    assert_eq!(data.table, "Contacts");
    assert_eq!(data.rows.len(), 2);
    assert_eq!(data.rows[0][1].text, "7");
    assert_eq!(data.rows[1][1].text, "8");
    assert_eq!(std::fs::read(&path).unwrap(), inserted);

    harness
        .get_by_role_and_label(Role::Label, "Field1, row 1: 7")
        .click_secondary();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Edit cell…")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "Replacement")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "Replacement")
        .type_text("42");
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Review change")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert_eq!(
        harness
            .state()
            .editing
            .review
            .as_ref()
            .unwrap()
            .1
            .target()
            .column,
        "Field1"
    );
    assert_eq!(std::fs::read(&path).unwrap(), inserted);
    assert!(!journal_path(&path).exists());

    harness
        .get_by_role_and_label(Role::Button, "Apply to original file")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(&mut harness);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.changed_rows == Some(1) && receipt.matched_rows == Some(1)),
        "{report:?}"
    );
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().rows[0][1].text,
        "7"
    );
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().rows[0][1].text,
        "42"
    );
    assert!(!journal_path(&path).exists());
    let edited = std::fs::read(&path).unwrap();
    assert_ne!(edited, inserted);

    harness
        .get_all_by_role(Role::RowHeader)
        .nth(1)
        .unwrap()
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(
        harness
            .state()
            .state
            .current_delete_target()
            .unwrap()
            .parent_ordinal,
        1
    );
    harness
        .get_by_role_and_label(Role::Button, "Delete")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Delete row")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(harness.state().editing.delete_review.is_some());
    assert_eq!(std::fs::read(&path).unwrap(), edited);
    assert!(!journal_path(&path).exists());
    harness
        .get_by_role_and_label(Role::Button, "Delete reviewed row")
        .click_accesskit();
    harness.run_steps(1);
    let report = finish_save(&mut harness);
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.changed_rows == Some(1) && receipt.matched_rows == Some(1)),
        "{report:?}"
    );
    assert_eq!(harness.state().state.data.as_ref().unwrap().rows.len(), 2);
    harness
        .get_by_role_and_label(Role::Button, "Close")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.data.as_ref().unwrap().rows.len(), 1);
    let deleted = std::fs::read(&path).unwrap();
    assert_ne!(deleted, edited);
    assert!(!journal_path(&path).exists());
    drop(harness);

    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    let data = reopened.state().state.data.as_ref().unwrap();
    assert_eq!(data.table, "Contacts");
    assert_eq!(data.rows.len(), 1);
    assert_eq!(data.rows[0][0].text, "1");
    assert_eq!(data.rows[0][1].text, "42");
    assert_eq!(std::fs::read(&path).unwrap(), deleted);
    assert!(!journal_path(&path).exists());
}
