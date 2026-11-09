//! Actual options widgets, consuming review, durable save and full-witness reopen.
use super::*;
use crate::workspace::test_support::{self, finish_reads};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use std::time::{Duration, Instant};
use tablecraft_testkit::fixtures::access::{canonical_creator_rename, canonical_database_options};
use tablecraft_ui::{ObjectKind, ObjectTarget};

#[test]
fn explicit_options_save_confirms_then_closes_persisted_tabs_and_keeps_scratch() {
    let before = canonical_creator_rename::originals().remove(0);
    let expected = canonical_database_options::explicit_off(&before);
    let (_directory, path, original, mut harness) = test_support::opened(before);
    assert_eq!(
        harness.state().state.database_options_available,
        tablecraft_runtime::SaveWorker::supports_database_options()
    );
    let persisted = harness.state().state.active_object().unwrap();
    let revision = persisted.target.document_revision;
    let scratch = harness
        .state_mut()
        .state
        .prepare_object_open(ObjectTarget {
            document_revision: revision,
            kind: ObjectKind::Sql,
        })
        .unwrap()
        .tab;
    let exact_text = "  SELECT 17 AS [Scratch]; -- app tool buffer\n";
    assert!(
        harness
            .state_mut()
            .state
            .replace_query_sql(exact_text.into())
    );
    harness.state_mut().activate_object(persisted.clone());
    finish_reads(&mut harness);
    harness.state_mut().state.show_options_dialog();
    harness.run_steps(2);
    let category = harness
        .state()
        .state
        .localizer
        .text("database-options-current-database");
    harness
        .get_by_role_and_label(Role::Button, &category)
        .click();
    harness.run_steps(2);
    finish_reads(&mut harness);
    assert_eq!(
        harness.state().state.database_options_observation(),
        Some(ViewState::Missing)
    );
    assert!(harness.state().state.can_disable_name_auto_correct());
    let disable = harness
        .state()
        .state
        .localizer
        .text("database-options-disable");
    harness
        .get_by_role_and_label(Role::Button, &disable)
        .click();
    harness.run_steps(2);
    // The UI-only close confirmation has not started any native action.
    assert!(harness.state().editing.database_options.is_none());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(
        harness
            .state()
            .state
            .object_tabs()
            .iter()
            .any(|tab| tab.id == persisted.id)
    );
    let confirm = harness
        .state()
        .state
        .localizer
        .text("database-options-close-continue");
    harness
        .get_by_role_and_label(Role::Button, &confirm)
        .click();
    harness.run_steps(1);
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness.state().state.busy || harness.state().editing.database_options.is_some() {
        assert!(
            Instant::now() < deadline,
            "Options actor/witness did not finish: {:?}",
            harness.state().state.error
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    finish_reads(&mut harness);
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    assert!(!journal_path(&path).exists());
    let tabs = harness.state().state.object_tabs();
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs[0].id, scratch.id);
    assert_ne!(tabs[0].target.document_revision, revision);
    assert_eq!(harness.state().state.sql, exact_text);
    assert!(!harness.state().state.native_source_stale());
}

#[test]
fn cancelling_readonly_options_review_releases_presentation_without_starting_save() {
    let before = canonical_creator_rename::originals().remove(0);
    let (_directory, path, original, mut harness) = test_support::opened(before);
    assert_eq!(
        harness.state().state.database_options_available,
        tablecraft_runtime::SaveWorker::supports_database_options()
    );
    let revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    harness.state_mut().inspect_database_options(revision);
    finish_reads(&mut harness);
    assert!(
        harness
            .state_mut()
            .state
            .request_database_options_close_confirmation()
    );
    let request = harness
        .state_mut()
        .state
        .approve_database_options_close()
        .unwrap();
    let tabs = harness.state().state.object_tabs();
    harness.state_mut().disable_name_auto_correct(request);
    let request_id = harness
        .state()
        .editing
        .database_options
        .as_ref()
        .unwrap()
        .request_id;
    harness
        .state_mut()
        .cancel_database_options_review(request_id);
    harness.run_steps(2);
    assert!(!harness.state().state.busy);
    assert!(harness.state().editing.database_options.is_none());
    assert!(harness.state().editing.save.is_none());
    assert!(harness.state().state.options_dialog_open());
    assert_eq!(harness.state().state.object_tabs(), tabs);
    assert_eq!(std::fs::read(&path).unwrap(), original);
    assert!(!journal_path(&path).exists());
}
