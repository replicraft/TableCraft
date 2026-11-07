//! Explicit Create → Table uses the real open/review/save host without windows.
use super::Desktop;
use super::test_support::start_save;
use crate::workspace::test_support::{app_harness, finish_reads, opened};
use egui::accesskit::Role;
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use std::time::{Duration, Instant};
use tablecraft_runtime::{SaveKnownOutcome, SaveOutcome, journal_path};
use tablecraft_testkit::{
    fixtures::access::{
        canonical_blank_creation, canonical_initial_table_creation, mutation_fixture,
    },
    support::access::restart::stage_first_table,
};
use tablecraft_ui::ObjectKind;

fn show_create(harness: &mut Harness<'_, Desktop>) {
    harness
        .get_by_role_and_label(Role::Tab, "Create")
        .click_accesskit();
    harness.run_steps(2);
}

fn create_table(harness: &mut Harness<'_, Desktop>) {
    assert!(harness.state().state.can_create_first_table());
    show_create(harness);
    let button = harness.get_by_role_and_label(Role::Button, "Table");
    assert!(!button.accesskit_node().is_disabled());
    button.click_accesskit();
    harness.run_steps(2);
    assert!(harness.state().state.has_unsaved_table_drafts());
    assert!(matches!(
        harness.state().state.active_object().unwrap().target.kind,
        ObjectKind::DraftTable { .. }
    ));
    // Create → Table is the Datasheet command, distinct from Table Design.
    harness.get_by_role_and_label(Role::Button, "Click to Add");
    assert!(harness.state().state.data.is_none());
    assert!(harness.state().state.schema.is_none());
}

#[test]
fn opening_blank_is_inert_and_explicit_ribbon_table_saves_to_the_same_path() {
    let original = canonical_blank_creation::source();
    let (_directory, path, _, mut harness) = opened(original.clone());
    assert!(harness.state().state.can_create_first_table());
    assert!(!harness.state().state.has_unsaved_table_drafts());
    assert!(harness.state().state.object_tabs().is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), original);

    create_table(&mut harness);
    harness
        .get_by_role_and_label(Role::Button, "Click to Add")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Short Text (255)")
        .click_accesskit();
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .focus();
    harness.run_steps(1);
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness
        .get_by_role_and_label(Role::TextInput, "Field name")
        .type_text("Name");
    harness.run_steps(1);
    harness
        .get_by_role_and_label(Role::Button, "Rename field")
        .click_accesskit();
    harness.run_steps(2);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!journal_path(&path).exists());
    assert!(harness.state().first_table.is_none());

    start_save(&mut harness, "Contacts");
    finish_reads(&mut harness);
    assert!(!harness.state().editing.has_save());
    assert!(!harness.state().state.has_unsaved_table_drafts());
    assert!(!harness.state().state.can_create_first_table());
    assert_eq!(harness.state().state.path, path.to_string_lossy());
    assert_ne!(std::fs::read(&path).unwrap(), original);
    drop(harness);

    let mut reopened = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut reopened);
    let data = reopened.state().state.data.as_ref().unwrap();
    assert_eq!(data.table, "Contacts");
    assert_eq!(
        data.columns
            .iter()
            .map(|column| column.name.as_str())
            .collect::<Vec<_>>(),
        ["ID", "Name"]
    );
    assert!(data.rows.is_empty());
    assert!(!reopened.state().state.can_create_first_table());
}

#[test]
fn readable_noncanonical_blank_and_populated_sources_keep_table_creation_disabled() {
    let mut noncanonical = canonical_blank_creation::source();
    *noncanonical.last_mut().unwrap() = 17;
    let existing_ace16 =
        canonical_initial_table_creation::source(canonical_initial_table_creation::Case::ShortText);
    for original in [
        noncanonical,
        existing_ace16.bytes,
        mutation_fixture::database(),
    ] {
        let (_directory, path, _, mut harness) = opened(original.clone());
        assert!(harness.state().state.database.is_some());
        assert!(!harness.state().state.can_create_first_table());
        let generation = harness.state().generation;
        show_create(&mut harness);
        let button = harness.get_by_role_and_label(Role::Button, "Table");
        assert!(button.accesskit_node().is_disabled());
        button.click_accesskit();
        harness.run_steps(2);
        assert!(!harness.state().state.has_unsaved_table_drafts());
        assert!(harness.state().first_table.is_none());
        assert_eq!(harness.state().generation, generation);
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert!(!journal_path(&path).exists());
    }
}

#[test]
fn rolled_back_blank_requires_explicit_create_after_recovery_acknowledgment() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("restart.accdb");
    let images = stage_first_table(&path, &journal_path(&path), false);
    let mut harness = app_harness(Desktop::new(egui::Context::default(), Some(path.clone())));
    finish_reads(&mut harness);
    assert!(harness.state().state.recovery_prompt_open());
    assert!(!harness.state().state.can_create_first_table());
    harness
        .get_by_role_and_label(Role::Button, "Recover")
        .click_accesskit();
    harness.run_steps(1);
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().state.recovery_pending() {
        assert!(Instant::now() < deadline, "recovery did not finish");
        harness.run_steps(1);
        std::thread::yield_now();
    }
    let report = harness
        .state()
        .editing
        .worker
        .poll()
        .expect("terminal recovery report");
    assert!(
        matches!(report.outcome, SaveOutcome::Saved { receipt } if receipt.known == SaveKnownOutcome::RolledBack)
    );
    assert_eq!(std::fs::read(&path).unwrap(), images.before);
    assert!(harness.state().state.database.is_none());
    harness
        .get_all_by_role_and_label(Role::Button, "Close")
        .find(|node| !node.accesskit_node().is_disabled())
        .expect("enabled recovery acknowledgment")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    assert!(!harness.state().state.has_unsaved_table_drafts());
    assert!(harness.state().state.can_create_first_table());
    create_table(&mut harness);
    assert_eq!(std::fs::read(&path).unwrap(), images.before);
}
