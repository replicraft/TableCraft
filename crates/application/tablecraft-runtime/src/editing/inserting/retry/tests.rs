use super::InsertRetryEvidence;
use crate::{
    SaveCompletion, SaveFailure, SaveFailureKind, SaveKnownOutcome, SaveOperation, SaveOutcome,
    SavePhase, SaveReceipt, SaveReport, SaveSessionState,
};

fn failed(
    current_outcome: Option<SaveKnownOutcome>,
    no_writes: bool,
    last_completed: Option<SaveReceipt>,
) -> SaveReport {
    SaveReport {
        id: 1,
        operation: SaveOperation::Apply,
        outcome: SaveOutcome::Failed(SaveFailure {
            kind: SaveFailureKind::Commit,
            phase: Some(if no_writes {
                SavePhase::Planning
            } else {
                SavePhase::Commit
            }),
            state: Some(if no_writes {
                SaveSessionState::Ready
            } else {
                SaveSessionState::RetainedJournal
            }),
            current_outcome,
            last_completed,
            can_recover: false,
            can_release: true,
        }),
    }
}

fn receipt(known: SaveKnownOutcome) -> SaveReceipt {
    SaveReceipt {
        completion: SaveCompletion::Applied,
        known,
        matched_rows: Some(1),
        changed_rows: Some(1),
        schema_change: None,
        retirement: None,
        cleanup: None,
    }
}

fn released() -> SaveReport {
    SaveReport {
        id: 2,
        operation: SaveOperation::Release,
        outcome: SaveOutcome::Released {
            current_outcome: None,
            last_completed: None,
        },
    }
}

#[test]
fn original_apply_no_writes_survives_resource_release_without_inventing_undo() {
    let mut evidence = InsertRetryEvidence::default();
    evidence.observe(failed(None, true, None));
    assert!(evidence.can_retry_original());
    evidence.observe(released());
    assert!(evidence.can_retry_original());
    let mut unknown = InsertRetryEvidence::default();
    unknown.observe(failed(None, false, None));
    unknown.observe(released());
    assert!(!unknown.can_retry_original());
}

#[test]
fn only_current_original_facts_enable_retry_and_committed_facts_are_sticky() {
    for known in [
        SaveKnownOutcome::RolledBack,
        SaveKnownOutcome::OriginalVerified,
        SaveKnownOutcome::Committed,
        SaveKnownOutcome::ReplacementVerified,
    ] {
        let mut evidence = InsertRetryEvidence::default();
        evidence.observe(failed(Some(known), false, None));
        evidence.observe(released());
        let original = matches!(
            known,
            SaveKnownOutcome::RolledBack | SaveKnownOutcome::OriginalVerified
        );
        assert_eq!(evidence.can_retry_original(), original);
        if !original {
            let mut later = failed(Some(SaveKnownOutcome::OriginalVerified), false, None);
            later.operation = SaveOperation::Recover;
            later.id = 3;
            evidence.observe(later);
            assert!(!evidence.can_retry_original());
        }
    }
}

#[test]
fn historical_receipts_are_not_current_outcomes_and_new_apply_starts_a_new_chain() {
    for historical in [
        SaveKnownOutcome::OriginalVerified,
        SaveKnownOutcome::Committed,
    ] {
        let mut evidence = InsertRetryEvidence::default();
        evidence.observe(failed(None, false, Some(receipt(historical))));
        evidence.observe(SaveReport {
            id: 2,
            operation: SaveOperation::Release,
            outcome: SaveOutcome::Released {
                current_outcome: None,
                last_completed: Some(receipt(historical)),
            },
        });
        assert!(!evidence.can_retry_original());
        let mut next = failed(None, true, None);
        next.id = 3;
        evidence.observe(next);
        assert!(evidence.can_retry_original());
    }
}
