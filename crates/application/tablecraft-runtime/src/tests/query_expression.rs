//! Lease-free saved-query review, consuming save ownership and guarded read reopening.
use super::{open, request, saved::opened, terminal};
use crate::{
    Command, Error, QueryExpressionReview, Response, ReviewedQueryExpression, SaveFailureKind,
    SaveOutcome, SaveQueryChange, SaveStartFailure, SaveWorker, SavedQueryTarget,
};
use tablecraft_testkit::{
    fixtures::access::saved_queries::fitting_select::storage_pair,
    support::access::retained_target::assert_retained_target_observation,
};

fn review(
    worker: &crate::Worker,
    target: &SavedQueryTarget,
    replacement: &str,
) -> (QueryExpressionReview, ReviewedQueryExpression) {
    let Response::QueryExpressionReviewed { review, capability } = request(
        worker,
        Command::ReviewQueryLiteralExpression {
            target: target.clone(),
            expected: "42".into(),
            replacement: replacement.into(),
        },
    )
    .unwrap() else {
        panic!("expected the qualified existing constant SELECT review");
    };
    assert_eq!(review.target, *target);
    assert_eq!(review.native_id, -2147483525);
    assert_eq!(review.expected, "42");
    assert_eq!(review.replacement, replacement);
    (review, capability)
}

#[test]
fn reviewed_query_and_noop_save_exact_images_and_reopen_without_execution_replay() {
    let [before, after] = storage_pair();
    for replacement in ["42", "43"] {
        let (directory, path, worker, target) = opened(&before);
        let (description, capability) = review(&worker, &target, replacement);
        let (_, second) = review(&worker, &target, "44");
        assert_eq!(description.changed, replacement == "43");
        assert_eq!(capability.source_path(), path);
        let witness = capability.reopen_witness();
        assert_eq!(witness.query_name(), "Saved");
        assert_eq!(witness.native_id(), -2147483525);
        assert_eq!(witness.document_revision(), target.document_revision);
        tablecraft_access::editing::offline::OfflineWriteSession::open(
            &path,
            crate::controls::SCOPE,
            &mut crate::controls::budget(),
            &|| false,
        )
        .unwrap()
        .release()
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let saves = SaveWorker::new(|| {});
        let bundle = directory.path().join("query.journal");
        let id = saves
            .start_query_expression(capability, bundle.clone())
            .unwrap();
        let rejected = saves
            .start_query_expression(second, directory.path().join("other.journal"))
            .unwrap_err();
        assert_eq!(rejected.kind, SaveStartFailure::Busy);
        assert!(rejected.into_reviewed().is_some());
        let report = terminal(&saves);
        let SaveOutcome::Saved { receipt } = report.outcome else {
            panic!("expected acknowledged query save: {report:?}");
        };
        assert_eq!(
            receipt.query_change,
            (replacement == "43").then_some(SaveQueryChange::ExpressionSaved)
        );
        assert_eq!(receipt.schema_change, None);
        assert_eq!((receipt.matched_rows, receipt.changed_rows), (None, None));
        assert_eq!(saves.poll(), Some(report));
        assert!(saves.acknowledge(id));
        let expected = if replacement == "43" { &after } else { &before };
        assert_eq!(std::fs::read(&path).unwrap(), expected.as_slice());
        let Response::Opened {
            document_revision,
            queries,
            ..
        } = request(&worker, Command::OpenQueryExpressionSaved { witness }).unwrap()
        else {
            panic!("expected read-only reopening of the exact saved image");
        };
        assert!(document_revision > target.document_revision);
        let query = queries.iter().find(|query| query.name == "Saved").unwrap();
        let fresh = SavedQueryTarget {
            document_revision,
            object_id: query.object_id,
            name: query.name.clone(),
        };
        let Response::Data { rows, .. } = request(
            &worker,
            Command::RunSavedQuery {
                options: Default::default(),
                target: fresh,
                parameters: Vec::new(),
            },
        )
        .unwrap() else {
            panic!("expected explicit saved query execution");
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0][0].text, replacement);
        assert_eq!(std::fs::read(&path).unwrap(), expected.as_slice());
    }
}

#[test]
fn source_drift_retains_the_target_without_journal_until_explicit_release() {
    let [before, _] = storage_pair();
    let (directory, path, worker, target) = opened(&before);
    let (_, capability) = review(&worker, &target, "43");
    let mut changed = before;
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &changed).unwrap();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("stale-query.journal");
    let id = saves
        .start_query_expression(capability, bundle.clone())
        .unwrap();
    let SaveOutcome::Failed(failure) = terminal(&saves).outcome else {
        panic!("expected full-source drift refusal");
    };
    assert_eq!(failure.kind, SaveFailureKind::SourceChanged);
    assert!(failure.can_release);
    assert!(!failure.can_recover);
    assert!(!saves.acknowledge(id));
    assert!(!bundle.exists());
    assert_retained_target_observation(&path, &changed);
    let released = saves.release(id).unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Released { .. }
    ));
    assert!(saves.acknowledge(released));
    assert_eq!(std::fs::read(&path).unwrap(), changed);
}

#[test]
fn document_replacement_invalidates_capability_and_old_query_target() {
    let [before, _] = storage_pair();
    let (directory, path, worker, target) = opened(&before);
    let (_, capability) = review(&worker, &target, "43");
    request(&worker, open(&path, "")).unwrap();
    let saves = SaveWorker::new(|| {});
    let bundle = directory.path().join("invalidated-query.journal");
    let error = saves
        .start_query_expression(capability, bundle.clone())
        .unwrap_err();
    assert_eq!(error.kind, SaveStartFailure::StaleDocument);
    assert!(error.into_reviewed().is_some());
    assert!(!bundle.exists());
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryLiteralExpression {
                target,
                expected: "42".into(),
                replacement: "43".into(),
            }
        ),
        Err(Error::StaleSavedQuery)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn guarded_reopen_requires_complete_saved_content_and_does_not_repeat_the_save() {
    let [before, mut after] = storage_pair();
    let (directory, path, worker, target) = opened(&before);
    let (_, capability) = review(&worker, &target, "43");
    let witness = capability.reopen_witness();
    let saves = SaveWorker::new(|| {});
    let id = saves
        .start_query_expression(capability, directory.path().join("query.journal"))
        .unwrap();
    assert!(matches!(
        terminal(&saves).outcome,
        SaveOutcome::Saved { .. }
    ));
    assert!(saves.acknowledge(id));
    *after.last_mut().unwrap() ^= 1;
    std::fs::write(&path, &after).unwrap();
    assert!(matches!(
        request(&worker, Command::OpenQueryExpressionSaved { witness }),
        Err(Error::QueryExpressionSourceChanged)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), after);
}

#[test]
fn wrong_object_identity_and_unsupported_expression_do_not_issue_a_capability() {
    let [before, _] = storage_pair();
    let (directory, path, worker, target) = opened(&before);
    let mut wrong = target.clone();
    wrong.object_id ^= 1;
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryLiteralExpression {
                target: wrong,
                expected: "42".into(),
                replacement: "43".into(),
            }
        ),
        Err(Error::StaleSavedQuery)
    ));
    assert!(matches!(
        request(
            &worker,
            Command::ReviewQueryLiteralExpression {
                target,
                expected: "42".into(),
                replacement: "NeverRunMe()".into(),
            }
        ),
        Err(Error::QueryExpression(_))
    ));
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}
