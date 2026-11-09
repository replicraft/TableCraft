use super::*;
use crate::editing::{
    QueryExpressionOutcomeError, SaveCompletion, SaveDatabaseOptionsChange, SaveFailure,
    SaveFailureKind, SaveKnownOutcome, SavePhase, SaveQueryChange, SaveSessionState,
};

fn receipt() -> SaveReceipt {
    SaveReceipt {
        completion: SaveCompletion::Applied,
        known: SaveKnownOutcome::Committed,
        matched_rows: None,
        changed_rows: None,
        schema_change: Some(SaveSchemaChange::ColumnAdded),
        query_change: None,
        database_options_change: None,
        retirement: None,
        cleanup: None,
    }
}

fn facts(outcome: SaveOutcome) -> (bool, Option<SaveReceipt>) {
    let facts = outcome.field_addition_reopen_facts().unwrap();
    (facts.replacement, facts.completed)
}

#[test]
fn column_and_query_contracts_refuse_each_others_replacement_scope() {
    let column = receipt();
    assert_eq!(
        facts(SaveOutcome::Saved { receipt: column }),
        (true, Some(column))
    );
    assert_eq!(
        SaveOutcome::Saved { receipt: column }.query_expression_reopen_facts(),
        Err(QueryExpressionOutcomeError::UnqualifiedReplacementReceipt)
    );
    let mut query = column;
    query.schema_change = None;
    query.query_change = Some(SaveQueryChange::ExpressionSaved);
    assert_eq!(
        SaveOutcome::Saved { receipt: query }.field_addition_reopen_facts(),
        Err(FieldAdditionOutcomeError::UnqualifiedReplacementReceipt)
    );
    assert!(
        SaveOutcome::Saved { receipt: query }
            .query_expression_reopen_facts()
            .unwrap()
            .replacement
    );
    for field in 0..6 {
        let mut wrong = column;
        match field {
            0 => wrong.schema_change = Some(SaveSchemaChange::TableRenamed),
            1 => wrong.query_change = Some(SaveQueryChange::ExpressionSaved),
            2 => wrong.matched_rows = Some(0),
            3 => wrong.changed_rows = Some(0),
            4 => wrong.schema_change = None,
            _ => {
                wrong.database_options_change =
                    Some(SaveDatabaseOptionsChange::NameAutoCorrectDisabled)
            }
        }
        assert_eq!(
            SaveOutcome::Saved { receipt: wrong }.field_addition_reopen_facts(),
            Err(FieldAdditionOutcomeError::UnqualifiedReplacementReceipt)
        );
    }
}

#[test]
fn current_original_precedes_history_and_released_after_never_invents_receipt() {
    for known in [
        SaveKnownOutcome::OriginalVerified,
        SaveKnownOutcome::RolledBack,
    ] {
        assert_eq!(
            facts(SaveOutcome::Released {
                current_outcome: Some(known),
                last_completed: Some(receipt()),
            }),
            (false, Some(receipt()))
        );
        let mut original = receipt();
        original.known = known;
        assert_eq!(
            facts(SaveOutcome::Saved { receipt: original }),
            (false, None)
        );
    }
    for known in [
        SaveKnownOutcome::Committed,
        SaveKnownOutcome::ReplacementVerified,
    ] {
        assert_eq!(
            facts(SaveOutcome::Released {
                current_outcome: Some(known),
                last_completed: None,
            }),
            (true, None)
        );
    }
    assert_eq!(
        facts(SaveOutcome::Released {
            current_outcome: None,
            last_completed: Some(receipt()),
        }),
        (true, Some(receipt()))
    );
    let mut wrong_history = receipt();
    wrong_history.schema_change = Some(SaveSchemaChange::TableRenamed);
    assert_eq!(
        facts(SaveOutcome::Released {
            current_outcome: None,
            last_completed: Some(wrong_history),
        }),
        (false, None)
    );
    assert_eq!(
        facts(SaveOutcome::Released {
            current_outcome: Some(SaveKnownOutcome::ReplacementVerified),
            last_completed: Some(wrong_history),
        }),
        (true, None)
    );
}

#[test]
fn failed_retained_owners_never_publish_even_with_known_after_and_receipt() {
    for phase in [
        SavePhase::Commit,
        SavePhase::Recovery,
        SavePhase::Finalization,
        SavePhase::Release,
    ] {
        let failed = SaveFailure {
            kind: SaveFailureKind::Finalization,
            phase: Some(phase),
            state: Some(SaveSessionState::RetainedJournal),
            current_outcome: Some(SaveKnownOutcome::Committed),
            last_completed: Some(receipt()),
            can_recover: true,
            can_release: true,
        };
        assert_eq!(facts(SaveOutcome::Failed(failed)), (false, None));
    }
}
