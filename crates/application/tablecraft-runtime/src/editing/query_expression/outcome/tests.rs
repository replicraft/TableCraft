use super::*;
use crate::editing::{
    SaveCompletion, SaveDatabaseOptionsChange, SaveFailure, SaveFailureKind, SavePhase,
    SaveSchemaChange, SaveSessionState,
};

fn facts(outcome: SaveOutcome) -> (bool, Option<SaveReceipt>) {
    let facts = outcome.query_expression_reopen_facts().unwrap();
    (facts.replacement, facts.completed)
}

fn receipt() -> SaveReceipt {
    SaveReceipt {
        completion: SaveCompletion::Applied,
        known: SaveKnownOutcome::Committed,
        matched_rows: None,
        changed_rows: None,
        schema_change: None,
        query_change: Some(SaveQueryChange::ExpressionSaved),
        database_options_change: None,
        retirement: None,
        cleanup: None,
    }
}

#[test]
fn only_separate_query_replacement_facts_admit_an_acknowledged_after_read() {
    let good = receipt();
    assert_eq!(
        facts(SaveOutcome::Saved { receipt: good }),
        (true, Some(good))
    );
    for field in 0..6 {
        let mut wrong = good;
        match field {
            0 => wrong.query_change = None,
            1 => wrong.schema_change = Some(SaveSchemaChange::TableRenamed),
            2 => wrong.matched_rows = Some(0),
            3 => wrong.changed_rows = Some(0),
            4 => wrong.known = SaveKnownOutcome::OriginalVerified,
            _ => {
                wrong.database_options_change =
                    Some(SaveDatabaseOptionsChange::NameAutoCorrectDisabled)
            }
        }
        let result = SaveOutcome::Saved { receipt: wrong }.query_expression_reopen_facts();
        if field == 4 {
            assert!(!result.unwrap().replacement);
        } else {
            assert_eq!(
                result,
                Err(QueryExpressionOutcomeError::UnqualifiedReplacementReceipt)
            );
        }
    }
}

#[test]
fn released_current_original_precedes_history_without_inventing_a_receipt() {
    for known in [
        SaveKnownOutcome::OriginalVerified,
        SaveKnownOutcome::RolledBack,
    ] {
        assert_eq!(
            facts(SaveOutcome::Released {
                current_outcome: Some(known),
                last_completed: Some(receipt())
            }),
            (false, Some(receipt()))
        );
    }
    assert_eq!(
        facts(SaveOutcome::Released {
            current_outcome: Some(SaveKnownOutcome::ReplacementVerified),
            last_completed: None
        }),
        (true, None)
    );
    assert_eq!(
        facts(SaveOutcome::Released {
            current_outcome: None,
            last_completed: Some(receipt())
        }),
        (true, Some(receipt()))
    );
}

#[test]
fn retained_failure_never_reopens_or_bypasses_its_recovery_owner() {
    let failure = SaveFailure {
        kind: SaveFailureKind::Finalization,
        phase: Some(SavePhase::Finalization),
        state: Some(SaveSessionState::RetainedJournal),
        current_outcome: Some(SaveKnownOutcome::Committed),
        last_completed: Some(receipt()),
        can_recover: true,
        can_release: true,
    };
    assert_eq!(facts(SaveOutcome::Failed(failure)), (false, None));
}
