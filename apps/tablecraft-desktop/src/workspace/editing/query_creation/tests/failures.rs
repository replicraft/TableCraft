//! Real actor failures and witnessed read retry, without injected outcomes.
mod return_to_draft;

use super::{name, opened, run_saved, saved, scratch, sql};
use crate::workspace::{Desktop, test_support::finish_save};
use egui::accesskit::Role;
use egui_kittest::{Harness, kittest::Queryable};
use std::time::{Duration, Instant};
use tablecraft_runtime::{
    QueryCreationReview, Response, ReviewedQueryCreation, SaveFailureKind, SaveOutcome, SaveReport,
    journal_path,
};
use tablecraft_ui::{Action, ObjectKind};

fn begin_review(harness: &mut Harness<'_, Desktop>) {
    // The ordinary OK button uses this same issued shared action. Pause at its
    // real service boundary so physical-source races do not depend on CPU timing.
    let Some(Action::ReviewQueryCreation { request }) =
        harness.state_mut().state.submit_query_creation_name()
    else {
        panic!("expected an actual name-bound shared request");
    };
    harness.state_mut().save_query_creation(request);
}

fn review(harness: &Harness<'_, Desktop>) -> (u64, QueryCreationReview, ReviewedQueryCreation) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(
            Instant::now() < deadline,
            "actual creation review must finish"
        );
        if let Some((generation, result)) = harness.state().worker.poll() {
            let Response::QueryCreationReviewed { review, capability } = result.unwrap() else {
                panic!("expected the genuine creation capability");
            };
            return (generation, review, capability);
        }
        std::thread::yield_now();
    }
}

fn report(harness: &Harness<'_, Desktop>) -> SaveReport {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(
            Instant::now() < deadline,
            "actual creation save must finish"
        );
        if let Some(report) = harness.state().editing.worker.poll() {
            return report;
        }
        std::thread::yield_now();
    }
}

#[test]
fn changed_source_keeps_real_failure_custody_until_explicit_release_and_ack() {
    let (_directory, path, case, clock, mut harness) = opened();
    saved(&mut harness, "Saved");
    let parked = harness.state().state.active_object().unwrap();
    scratch(&mut harness);
    sql(&mut harness, "SELECT 99 AS Answer;");
    let tab = harness.state().state.active_object().unwrap();
    name(&mut harness, false);
    begin_review(&mut harness);
    let (generation, review, capability) = review(&harness);
    let mut changed = case.before;
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    harness
        .state_mut()
        .start_reviewed_query_creation(generation, review, capability);
    let report = finish_save(&mut harness);
    let SaveOutcome::Failed(failure) = report.outcome else {
        panic!("actual exclusive source drift must fail before publication: {report:?}");
    };
    assert_eq!(failure.kind, SaveFailureKind::SourceChanged);
    assert!(failure.can_release);
    assert!(!failure.can_recover);
    let (identity, _) = harness.state().state.query_creation_save_status().unwrap();
    assert!(
        harness
            .state()
            .state
            .can_resolve_query_creation_save(identity, false)
    );
    assert!(
        !harness
            .state()
            .state
            .can_resolve_query_creation_save(identity, true)
    );
    assert!(
        !harness
            .state()
            .state
            .can_acknowledge_query_creation_save(identity)
    );
    assert!(harness.state().editing.has_save());
    harness.state_mut().activate_object(parked);
    assert_eq!(harness.state().state.active_object().as_ref(), Some(&tab));
    assert!(
        harness.state().editing.has_save(),
        "blocked tab activation cannot discard terminal custody"
    );
    assert_eq!(harness.state().state.sql, "SELECT 99 AS Answer;");
    assert_eq!(clock.calls(), 1);
    assert_eq!(std::fs::read(&path).unwrap(), changed);
    assert!(!journal_path(&path).exists());

    let release = harness.state().state.localizer.text("editing-release");
    harness
        .get_by_role_and_label(Role::Button, &release)
        .click_accesskit();
    harness.run_steps(1);
    let released = finish_save(&mut harness);
    assert!(matches!(
        released.outcome,
        SaveOutcome::Released {
            current_outcome: None,
            last_completed: None,
        }
    ));
    assert!(harness.state().editing.has_save());
    let close = harness.state().state.localizer.text("backstage-close");
    harness
        .get_by_role_and_label(Role::Button, &close)
        .click_accesskit();
    harness.run_steps(2);
    assert!(!harness.state().editing.has_save());
    assert_eq!(harness.state().state.sql, "SELECT 99 AS Answer;");
    assert!(
        !harness
            .state()
            .state
            .database
            .as_ref()
            .unwrap()
            .queries
            .iter()
            .any(|query| query.name == "Created")
    );
    assert_eq!(clock.calls(), 1);
    assert_eq!(std::fs::read(path).unwrap(), changed);
}

#[test]
fn foreign_after_blocks_publication_then_explicit_read_retry_resolves_the_real_saved_fact() {
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

    // NO_CLONE: This external writer mutates an independent image; the original
    // raw AFTER stays available for explicit restoration and complete verification.
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
            "guarded reopen must refuse foreign image"
        );
        harness.run_steps(1);
        std::thread::yield_now();
    }
    assert!(harness.state().editing.has_save());
    assert!(
        harness.state().editing.save.is_none(),
        "native actor was acknowledged, retained read witness was not discarded"
    );
    assert_eq!(harness.state().state.active_object().as_ref(), Some(&tab));
    assert!(matches!(
        harness.state().state.active_object().unwrap().target.kind,
        ObjectKind::Sql
    ));
    assert_eq!(harness.state().state.sql, "SELECT 99 AS Answer;");
    assert_eq!(std::fs::read(&path).unwrap(), foreign);
    assert_eq!(clock.calls(), 1);
    assert!(!journal_path(&path).exists());

    // Explicit external restoration belongs to this test. Retry itself receives
    // only the already accepted read witness and cannot repair/replay the save.
    std::fs::write(&path, &case.expected).unwrap();
    let retry = harness
        .state()
        .state
        .localizer
        .text("saved-query-save-retry-reopen");
    harness
        .get_by_role_and_label(Role::Button, &retry)
        .click_accesskit();
    harness.run_steps(1);
    crate::workspace::test_support::finish_schema_save(&mut harness);
    assert!(!harness.state().editing.has_save());
    assert_eq!(harness.state().state.active_object().unwrap().id, tab.id);
    run_saved(&mut harness, "99");
    saved(&mut harness, "Saved");
    run_saved(&mut harness, "42");
    assert_eq!(std::fs::read(path).unwrap(), case.expected);
    assert_eq!(clock.calls(), 1);
}
