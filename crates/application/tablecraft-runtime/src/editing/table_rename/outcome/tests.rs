use super::*;
use crate::editing::{
    SaveCompletion, SaveDatabaseOptionsChange, SaveKnownOutcome, SaveQueryChange,
};

fn receipt() -> SaveReceipt {
    SaveReceipt {
        completion: SaveCompletion::Applied,
        known: SaveKnownOutcome::Committed,
        matched_rows: None,
        changed_rows: None,
        schema_change: Some(SaveSchemaChange::TableRenamed),
        query_change: None,
        database_options_change: None,
        retirement: None,
        cleanup: None,
    }
}

#[test]
fn only_exact_rename_scope_can_supply_a_saved_replacement_receipt() {
    let expected = receipt();
    let facts = SaveOutcome::Saved { receipt: expected }
        .table_rename_reopen_facts()
        .unwrap();
    assert!(facts.replacement);
    assert_eq!(facts.completed, Some(expected));
    for case in 0..5 {
        let mut wrong = expected;
        match case {
            0 => wrong.schema_change = Some(SaveSchemaChange::ColumnRenamed),
            1 => wrong.matched_rows = Some(0),
            2 => wrong.changed_rows = Some(0),
            3 => wrong.query_change = Some(SaveQueryChange::ExpressionSaved),
            _ => {
                wrong.database_options_change =
                    Some(SaveDatabaseOptionsChange::NameAutoCorrectDisabled)
            }
        }
        assert!(
            SaveOutcome::Saved { receipt: wrong }
                .table_rename_reopen_facts()
                .is_err()
        );
    }
}

#[test]
fn released_current_original_overrides_history_and_after_needs_no_fake_receipt() {
    for known in [
        SaveKnownOutcome::OriginalVerified,
        SaveKnownOutcome::RolledBack,
    ] {
        let facts = SaveOutcome::Released {
            current_outcome: Some(known),
            last_completed: Some(receipt()),
        }
        .table_rename_reopen_facts()
        .unwrap();
        assert!(!facts.replacement);
        assert_eq!(facts.completed, Some(receipt()));
    }
    for known in [
        SaveKnownOutcome::Committed,
        SaveKnownOutcome::ReplacementVerified,
    ] {
        let facts = SaveOutcome::Released {
            current_outcome: Some(known),
            last_completed: None,
        }
        .table_rename_reopen_facts()
        .unwrap();
        assert!(facts.replacement);
        assert!(facts.completed.is_none());
    }
    let mut unrelated = receipt();
    unrelated.schema_change = Some(SaveSchemaChange::ColumnAdded);
    let facts = SaveOutcome::Released {
        current_outcome: None,
        last_completed: Some(unrelated),
    }
    .table_rename_reopen_facts()
    .unwrap();
    assert!(!facts.replacement);
    assert!(facts.completed.is_none());
    assert!(
        SaveOutcome::Uncertain {
            last_known_outcome: Some(SaveKnownOutcome::Committed),
            last_completed: Some(receipt())
        }
        .table_rename_reopen_facts()
        .is_err()
    );
}

#[test]
fn failed_owner_never_promotes_its_known_after_or_historical_receipt() {
    use crate::editing::{SaveFailure, SaveFailureKind, SaveSessionState};
    let outcome = SaveOutcome::Failed(SaveFailure {
        kind: SaveFailureKind::Finalization,
        phase: None,
        state: Some(SaveSessionState::RetainedJournal),
        current_outcome: Some(SaveKnownOutcome::ReplacementVerified),
        last_completed: Some(receipt()),
        can_recover: true,
        can_release: true,
    });
    let facts = outcome.table_rename_reopen_facts().unwrap();
    assert!(!facts.replacement);
    assert!(facts.completed.is_none());
}
