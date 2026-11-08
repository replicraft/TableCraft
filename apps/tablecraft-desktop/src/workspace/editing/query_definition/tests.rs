//! Real host capability composition; complete bytes remain independently authored.
mod guards;
mod interaction;
mod workspace;

use super::*;
use crate::workspace::test_support::{finish_reads, finish_save, opened};
use egui_kittest::{Harness, kittest::Queryable};
use tablecraft_runtime::{SaveFailureKind, SaveQueryChange};
use tablecraft_testkit::fixtures::access::saved_queries::fitting_select::storage_pair;
use tablecraft_ui::{ObjectKind, ObjectTarget};

fn open_query(harness: &mut Harness<'_, Desktop>) {
    for tab in harness.state().state.object_tabs() {
        harness.state_mut().close_object(tab);
    }
    let database = harness.state().state.database.as_ref().unwrap();
    let target = ObjectTarget {
        document_revision: database.document_revision,
        kind: ObjectKind::SavedQuery {
            object_id: database.queries[0].object_id,
            name: database.queries[0].name.clone(),
        },
    };
    harness.state_mut().open_object(target);
    finish_reads(harness);
    assert!(harness.state().state.saved_query_sql_edit_available);
}

fn query(harness: &mut Harness<'_, Desktop>) {
    open_query(harness);
    assert!(
        harness
            .state_mut()
            .state
            .set_saved_query_sql_text(" select  43 as [Answer];\n".into())
    );
}

fn take_review_response(
    harness: &Harness<'_, Desktop>,
) -> (u64, QuerySqlReview, ReviewedQueryExpression) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        assert!(
            std::time::Instant::now() < deadline,
            "SQL review did not finish"
        );
        if let Some((generation, result)) = harness.state().worker.poll() {
            let tablecraft_runtime::Response::QuerySqlReviewed { review, capability } =
                result.unwrap()
            else {
                panic!("expected the exact saved SQL runtime review");
            };
            return (generation, review, capability);
        }
        std::thread::yield_now();
    }
}

#[test]
fn exact_sql_stays_dirty_until_acknowledged_full_witness_reopen_and_revision_rebind() {
    let [before, after] = storage_pair();
    let (_directory, path, _, mut harness) = opened(before);
    query(&mut harness);
    let old_revision = harness
        .state()
        .state
        .database
        .as_ref()
        .unwrap()
        .document_revision;
    let request = harness
        .state()
        .state
        .saved_query_sql_review_request()
        .unwrap();
    let draft_revision = request.draft_revision;
    harness.state_mut().save_query_definition(request);
    let report = finish_save(&mut harness);
    assert!(matches!(report.outcome, SaveOutcome::Saved { receipt }
        if receipt.query_change == Some(SaveQueryChange::ExpressionSaved)
            && receipt.schema_change.is_none() && receipt.database_options_change.is_none()
            && receipt.matched_rows.is_none() && receipt.changed_rows.is_none()));
    assert!(harness.state().state.has_modified_saved_query_sql());
    assert!(harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), after);
    harness.state_mut().acknowledge_query_definition(report.id);
    assert!(harness.state().state.has_modified_saved_query_sql());
    finish_reads(&mut harness);
    let state = &harness.state().state;
    let (original, proposed, revision, _) = state.saved_query_sql().unwrap();
    assert_eq!(original, " select  43 as [Answer];\n");
    assert_eq!(proposed, original);
    assert!(revision > draft_revision);
    assert!(state.database.as_ref().unwrap().document_revision > old_revision);
    assert!(!state.has_modified_saved_query_sql());
    assert!(!harness.state().editing.has_save());
    assert!(!journal_path(&path).exists());
}

#[test]
fn stale_target_release_preserves_dirty_original_and_proposed_text_without_retry() {
    let [before, _] = storage_pair();
    let (_directory, path, _, mut harness) = opened(before.clone());
    query(&mut harness);
    let request = harness
        .state()
        .state
        .saved_query_sql_review_request()
        .unwrap();
    let expected_original = request.original_sql.clone().unwrap();
    let expected_proposed = request.proposed_sql.clone();
    let expected_revision = request.draft_revision;
    harness.state_mut().save_query_definition(request);
    // Observe the genuine runtime review before the host consumes it, making
    // the stale-original precondition deterministic rather than a worker race.
    let (generation, review, capability) = take_review_response(&harness);
    let mut changed = before;
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    harness
        .state_mut()
        .start_reviewed_query_definition(generation, review, capability);
    let report = finish_save(&mut harness);
    assert!(
        matches!(report.outcome, SaveOutcome::Failed(failure) if failure.kind == SaveFailureKind::SourceChanged && failure.can_release && !failure.can_recover)
    );
    assert!(!journal_path(&path).exists());
    harness.state_mut().resolve_save(report.id, false);
    let released = finish_save(&mut harness);
    assert!(matches!(released.outcome, SaveOutcome::Released { .. }));
    harness
        .state_mut()
        .acknowledge_query_definition(released.id);
    let (original, proposed, revision, _) = harness.state().state.saved_query_sql().unwrap();
    assert_eq!(original, expected_original);
    assert_eq!(proposed, expected_proposed);
    assert_eq!(revision, expected_revision);
    assert!(harness.state().state.has_modified_saved_query_sql());
    assert!(!harness.state().editing.has_save());
    assert_eq!(std::fs::read(&path).unwrap(), changed);
}

#[test]
fn cancelling_unlocked_review_preserves_exact_text_and_creates_no_save_owner() {
    let [before, _] = storage_pair();
    let (_directory, path, _, mut harness) = opened(before.clone());
    query(&mut harness);
    let request = harness
        .state()
        .state
        .saved_query_sql_review_request()
        .unwrap();
    let expected = request.proposed_sql.clone();
    harness.state_mut().save_query_definition(request);
    assert!(harness.state_mut().cancel_query_definition_operation());
    assert!(!harness.state().editing.has_save());
    assert_eq!(harness.state().state.saved_query_sql().unwrap().1, expected);
    assert!(harness.state().state.has_modified_saved_query_sql());
    assert!(!journal_path(&path).exists());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn failed_witness_reopen_keeps_saved_terminal_facts_and_all_dirty_text() {
    let [before, after] = storage_pair();
    let (_directory, path, _, mut harness) = opened(before);
    query(&mut harness);
    let request = harness
        .state()
        .state
        .saved_query_sql_review_request()
        .unwrap();
    let expected_original = request.original_sql.clone().unwrap();
    let expected_proposed = request.proposed_sql.clone();
    let expected_revision = request.draft_revision;
    harness.state_mut().save_query_definition(request);
    let report = finish_save(&mut harness);
    assert!(matches!(report.outcome, SaveOutcome::Saved { .. }));
    // Only the terminal envelope is substituted to exercise retained cleanup
    // presentation. No retained-artifact filesystem qualification is inferred.
    let mut envelope = report;
    if let SaveOutcome::Saved { receipt } = &mut envelope.outcome {
        receipt.cleanup = Some(tablecraft_runtime::SaveCleanupDisposition::RetainedArtifact);
    }
    harness.state_mut().editing.save.as_mut().unwrap().report = Some(envelope);
    let mut changed = after;
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    harness.state_mut().acknowledge_query_definition(report.id);
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
            "witnessed SQL reopen did not finish"
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    let identity = harness
        .state()
        .editing
        .query_definition
        .as_ref()
        .unwrap()
        .status
        .unwrap();
    assert!(
        harness
            .state()
            .state
            .saved_query_sql_reopen_error(identity)
            .is_some()
    );
    assert!(
        harness
            .state()
            .state
            .can_return_to_saved_query_sql_draft(identity)
    );
    let (original, proposed, revision, _) = harness.state().state.saved_query_sql().unwrap();
    assert_eq!(original, expected_original);
    assert_eq!(proposed, expected_proposed);
    assert_eq!(revision, expected_revision);
    assert!(harness.state().state.has_modified_saved_query_sql());
    assert_eq!(std::fs::read(&path).unwrap(), changed);
    assert!(
        harness
            .state_mut()
            .keep_query_definition_dirty_after_reopen_failure()
    );
    assert!(!harness.state().editing.has_save());
    assert!(harness.state().state.has_modified_saved_query_sql());
    let saved_warning = harness
        .state()
        .state
        .localizer
        .text("editing-save-completed");
    assert!(
        harness
            .state()
            .state
            .error
            .as_ref()
            .unwrap()
            .contains(&saved_warning)
    );
    harness.run_steps(1);
    let cleanup = harness
        .state()
        .state
        .localizer
        .text("editing-save-retained-private-journal");
    harness.get_by_label(&cleanup);
}

#[test]
fn formatting_only_save_requires_fresh_witness_and_never_starts_native_persistence() {
    let [before, _] = storage_pair();
    let (_directory, path, _, mut harness) = opened(before.clone());
    query(&mut harness);
    let submitted = " select  42 as [Answer];\n";
    assert!(
        harness
            .state_mut()
            .state
            .set_saved_query_sql_text(submitted.into())
    );
    let saved = harness.state().state.active_object().unwrap();
    let scratch_text = "-- retain this other draft\n SELECT 99 AS [Scratch];\n";
    harness.state_mut().open_object(ObjectTarget {
        document_revision: saved.target.document_revision,
        kind: ObjectKind::Sql,
    });
    let scratch = harness.state().state.active_object().unwrap();
    harness.state_mut().state.sql = scratch_text.into();
    harness.state_mut().activate_object(saved);
    finish_reads(&mut harness);
    let tab_ids = harness
        .state()
        .state
        .object_tabs()
        .iter()
        .map(|tab| tab.id)
        .collect::<Vec<_>>();
    let request = harness
        .state()
        .state
        .saved_query_sql_review_request()
        .unwrap();
    let revision = request.draft_revision;
    harness.state_mut().save_query_definition(request);
    finish_reads(&mut harness);
    assert!(harness.state().editing.save.is_none());
    assert!(!harness.state().editing.worker.is_busy());
    assert!(!harness.state().editing.has_save());
    let (original, proposed, fresh_revision, _) = harness.state().state.saved_query_sql().unwrap();
    assert_eq!(original, submitted);
    assert_eq!(proposed, submitted);
    assert!(fresh_revision > revision);
    assert!(!harness.state().state.has_modified_saved_query_sql());
    assert!(!journal_path(&path).exists());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(
        harness
            .state()
            .state
            .object_tabs()
            .iter()
            .map(|tab| tab.id)
            .collect::<Vec<_>>(),
        tab_ids
    );
    let scratch = harness
        .state()
        .state
        .object_tabs()
        .into_iter()
        .find(|tab| tab.id == scratch.id)
        .unwrap();
    harness.state_mut().activate_object(scratch);
    finish_reads(&mut harness);
    assert_eq!(harness.state().state.sql, scratch_text);
    assert_eq!(
        std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
        1
    );
}
