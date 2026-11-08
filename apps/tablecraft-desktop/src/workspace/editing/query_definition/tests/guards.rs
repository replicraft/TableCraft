//! Stale close decisions and unchanged-read failures retain their exact owners.
use super::{open_query, query, storage_pair, take_review_response};
use crate::workspace::test_support::opened;

#[test]
fn stale_close_save_cannot_retag_an_existing_accepted_sql_lifecycle() {
    let [before, _] = storage_pair();
    let (_directory, path, _, mut harness) = opened(before.clone());
    query(&mut harness);
    let request = harness
        .state()
        .state
        .saved_query_sql_review_request()
        .unwrap();
    let stale = harness
        .state()
        .state
        .saved_query_sql_review_request()
        .unwrap();
    harness.state_mut().save_query_definition(request);
    let owner = harness
        .state()
        .editing
        .query_definition
        .as_ref()
        .unwrap()
        .request_id;
    harness.state_mut().save_query_definition_for_close(stale);
    let host = harness.state().editing.query_definition.as_ref().unwrap();
    assert_eq!(host.request_id, owner);
    assert!(!host.close_after_save);
    assert!(harness.state_mut().cancel_query_definition_operation());
    assert!(harness.state().state.has_modified_saved_query_sql());
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn unchanged_review_witness_failure_never_invents_saved_or_original_facts() {
    let [before, _] = storage_pair();
    let (_directory, path, _, mut harness) = opened(before.clone());
    open_query(&mut harness);
    let submitted = " select  42 as [Answer];\n";
    assert!(
        harness
            .state_mut()
            .state
            .set_saved_query_sql_text(submitted.into())
    );
    let request = harness
        .state()
        .state
        .saved_query_sql_review_request()
        .unwrap();
    let original = request.original_sql.clone().unwrap();
    let revision = request.draft_revision;
    harness.state_mut().save_query_definition(request);
    let (generation, review, capability) = take_review_response(&harness);
    assert!(!review.expression.changed);
    let mut changed = before;
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    harness
        .state_mut()
        .start_reviewed_query_definition(generation, review, capability);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while harness
        .state()
        .editing
        .query_definition
        .as_ref()
        .unwrap()
        .reopening
    {
        assert!(
            std::time::Instant::now() < deadline,
            "unchanged read did not finish"
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    let host = harness.state().editing.query_definition.as_ref().unwrap();
    assert!(host.unchanged_review);
    assert!(host.terminal.is_none());
    assert!(harness.state().editing.save.is_none());
    assert!(!harness.state().editing.worker.is_busy());
    assert!(
        harness
            .state_mut()
            .keep_query_definition_dirty_after_reopen_failure()
    );
    let (baseline, proposed, retained_revision, _) =
        harness.state().state.saved_query_sql().unwrap();
    assert_eq!(baseline, original);
    assert_eq!(proposed, submitted);
    assert_eq!(retained_revision, revision);
    assert!(harness.state().state.has_modified_saved_query_sql());
    let saved = harness
        .state()
        .state
        .localizer
        .text("editing-save-completed");
    let error = harness.state().state.error.as_ref().unwrap();
    assert!(!error.contains(&saved));
    assert!(!error.is_empty());
    assert_eq!(std::fs::read(&path).unwrap(), changed);
    assert!(!tablecraft_runtime::journal_path(&path).exists());
    // This flow intentionally leaves the old dirty view visible; no fresh read
    // or execution is implicitly retried after returning to the editor.
    assert!(!harness.state().state.busy);
}
