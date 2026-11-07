use super::*;
use crate::workspace::test_support::{app_harness, finish_reads, opened};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_testkit::fixtures::access::{canonical_blank_creation, mutation_fixture};
use tablecraft_ui::ObjectKind;

fn show_new(harness: &mut Harness<'_, Desktop>) {
    if harness.state().state.database.is_some() {
        harness
            .get_by_role_and_label(Role::Button, "File")
            .click_accesskit();
        harness.run_steps(3);
        harness
            .get_by_role_and_label(Role::Tab, "New")
            .click_accesskit();
        harness.run_steps(3);
    }
    harness
        .get_by_role_and_label(Role::Button, "Blank database")
        .click_accesskit();
    harness.run_steps(3);
}

#[test]
fn file_new_creates_and_opens_the_real_blank_database_then_runs_a_query() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("New database.accdb");
    let mut harness = app_harness(Desktop::new(egui::Context::default(), None));
    harness.run_steps(3);
    show_new(&mut harness);
    harness.state_mut().creation_selected(Some(path.clone()));
    harness.run_steps(3);
    assert!(!path.exists(), "choosing a destination must not publish it");
    harness
        .get_by_role_and_label(Role::Button, "Create")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);

    let original = std::fs::read(&path).unwrap();
    assert_eq!(original, canonical_blank_creation::source());
    let database = harness.state().state.database.as_ref().unwrap();
    assert_eq!(database.format, "Ace2016");
    assert!(database.tables.iter().all(|table| table.system));
    assert!(harness.state().state.data.is_none());
    assert!(harness.state().state.has_unsaved_table_drafts());
    assert!(!harness.state().state.has_modified_table_drafts());
    assert!(matches!(
        harness.state().state.active_object().unwrap().target.kind,
        ObjectKind::DraftTable { .. }
    ));
    let terminal = harness.state().creating.terminal.as_ref().unwrap();
    assert!(matches!(terminal.outcome(), RuntimeOutcome::Created(_)));
    assert!(!harness.state().creating.is_busy());

    harness
        .get_by_role_and_label(Role::Button, "New SQL query")
        .click_accesskit();
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
        .focus();
    harness.run_steps(1);
    harness
        .get_by_role_and_label(Role::MultilineTextInput, "SQL query")
        .type_text("SELECT 1 AS One");
    harness.run_steps(2);
    harness
        .get_by_role_and_label(Role::Button, "Run query")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);
    let result = harness.state().state.query_data.as_ref().unwrap();
    assert_eq!(result.rows[0][0].text, "1");
    assert_eq!(std::fs::read(&path).unwrap(), original);
}

#[test]
fn existing_destination_failure_keeps_the_current_document_and_original_bytes() {
    let original = mutation_fixture::with_rows(&[(1, 10)]);
    let (directory, path, original, mut harness) = opened(original);
    let title = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .title
        .clone();
    let rows = harness.state().state.data.as_ref().unwrap().rows.as_ptr();
    show_new(&mut harness);
    harness.state_mut().creation_selected(Some(path.clone()));
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Button, "Create")
        .click_accesskit();
    harness.run_steps(1);
    finish_reads(&mut harness);

    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    assert_eq!(
        harness.state().state.database.as_ref().unwrap().title,
        title
    );
    assert_eq!(
        harness.state().state.data.as_ref().unwrap().rows.as_ptr(),
        rows
    );
    let report = harness.state().creating.terminal.as_ref().unwrap();
    assert!(matches!(report.outcome(), RuntimeOutcome::Failed(_)));
    assert!(!harness.state().creating.is_busy());
    assert!(harness.query_by_label("No database was created.").is_some());
}

#[test]
fn cancelled_location_selection_has_no_creation_operation_or_output() {
    let directory = tempfile::tempdir().unwrap();
    let mut harness = app_harness(Desktop::new(egui::Context::default(), None));
    harness.run_steps(3);
    show_new(&mut harness);
    let destination = harness.state().state.creation_destination().to_owned();
    harness.state_mut().creation_selected(None);
    harness.run_steps(3);
    assert_eq!(harness.state().state.creation_destination(), destination);
    assert!(!harness.state().state.creation_is_pending());
    assert!(!harness.state().creating.is_busy());
    assert!(harness.state().creating.terminal.is_none());
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[test]
fn relative_destination_rejection_is_visible_and_never_starts_publication() {
    let mut harness = app_harness(Desktop::new(egui::Context::default(), None));
    harness.run_steps(3);
    show_new(&mut harness);
    harness
        .state_mut()
        .state
        .set_creation_destination("./relative.accdb".into());
    harness.run_steps(3);
    harness
        .get_by_role_and_label(Role::Button, "Create")
        .click_accesskit();
    harness.run_steps(3);
    assert!(!harness.state().state.creation_is_pending());
    assert!(!harness.state().creating.is_busy());
    assert!(harness.state().creating.terminal.is_none());
    assert!(
        harness
            .query_by_label("Choose a destination before creating the database.")
            .is_some()
    );
}
