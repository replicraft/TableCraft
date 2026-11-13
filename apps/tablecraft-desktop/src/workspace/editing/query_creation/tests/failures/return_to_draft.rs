//! Returning after a real failed witness read preserves the saved historical fact.
use super::super::{name, opened, scratch, sql};
use super::{begin_review, report, review};
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use std::time::{Duration, Instant};
use tablecraft_runtime::journal_path;
use tablecraft_ui::ObjectKind;

#[test]
fn return_to_draft_after_failed_guarded_reopen_retires_status_without_replay_or_false_publication()
{
    let (_directory, path, case, clock, mut harness) = opened();
    scratch(&mut harness);
    sql(&mut harness, "SELECT 99 AS Answer;");
    let tab = harness.state().state.active_object().unwrap();
    name(&mut harness, false);
    begin_review(&mut harness);
    let (generation, review, capability) = review(&harness);
    harness
        .state_mut()
        .start_reviewed_query_creation(generation, review, capability);
    let completed = report(&harness);
    assert!(
        completed
            .outcome
            .query_creation_reopen_facts()
            .unwrap()
            .replacement
    );
    assert_eq!(std::fs::read(&path).unwrap(), case.expected);

    // NO_CLONE: The external writer mutates an independent image to cause an
    // actual witness refusal; the literal AFTER remains available for restoration.
    let mut foreign = case.expected.clone();
    *foreign.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &foreign).unwrap();
    harness.state_mut().poll_saves();
    let (identity, _) = harness.state().state.query_creation_save_status().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while harness
        .state()
        .state
        .query_creation_reopen_error(identity)
        .is_none()
    {
        assert!(
            Instant::now() < deadline,
            "real witnessed read must finish with refusal"
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    assert!(
        harness
            .state()
            .state
            .can_return_to_query_creation_draft(identity)
    );
    assert!(harness.state().editing.save.is_none());
    assert!(harness.state().editing.has_save());
    assert!(harness.state().editing.worker.poll().is_none());
    assert_eq!(clock.calls(), 1);

    // This explicit external restoration precedes Return to draft. That command
    // must preserve the existing full AFTER without issuing either a save or read.
    std::fs::write(&path, &case.expected).unwrap();
    let generation = harness.state().generation;
    let label = harness
        .state()
        .state
        .localizer
        .text("saved-query-save-return-draft");
    harness
        .get_by_role_and_label(Role::Button, &label)
        .click_accesskit();
    harness.run_steps(2);
    assert!(!harness.state().editing.has_save());
    assert!(harness.state().state.query_creation_save_status().is_none());
    assert!(!harness.state().state.busy);
    assert_eq!(harness.state().generation, generation);
    assert_eq!(harness.state().state.active_object().as_ref(), Some(&tab));
    assert!(matches!(
        harness.state().state.active_object().unwrap().target.kind,
        ObjectKind::Sql
    ));
    assert_eq!(harness.state().state.sql, "SELECT 99 AS Answer;");
    assert!(
        harness
            .state()
            .state
            .error
            .as_ref()
            .is_some_and(|message| !message.is_empty())
    );
    let database = harness.state().state.database.as_ref().unwrap();
    assert_eq!(database.document_revision, tab.target.document_revision);
    assert!(!database.queries.iter().any(|query| query.name == "Created"));
    assert_eq!(clock.calls(), 1);
    assert_eq!(std::fs::read(&path).unwrap(), case.expected);
    assert!(!journal_path(&path).exists());
    assert!(harness.state().editing.worker.poll().is_none());
}
