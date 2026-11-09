use super::*;
use crate::editing::{SaveCompletion, SaveKnownOutcome, SaveQueryChange, SaveSchemaChange};

fn receipt() -> SaveReceipt {
    SaveReceipt {
        completion: SaveCompletion::Applied,
        known: SaveKnownOutcome::Committed,
        matched_rows: None,
        changed_rows: None,
        schema_change: None,
        query_change: None,
        database_options_change: Some(SaveDatabaseOptionsChange::NameAutoCorrectDisabled),
        retirement: None,
        cleanup: None,
    }
}

#[test]
fn settings_receipts_are_separate_from_schema_queries_and_record_counts() {
    let settings = receipt();
    let facts = SaveOutcome::Saved { receipt: settings }
        .database_options_reopen_facts()
        .unwrap();
    assert!(facts.replacement);
    assert_eq!(facts.completed, Some(settings));
    assert!(
        SaveOutcome::Saved { receipt: settings }
            .field_addition_reopen_facts()
            .is_err()
    );
    assert!(
        SaveOutcome::Saved { receipt: settings }
            .query_expression_reopen_facts()
            .is_err()
    );

    let invalid = [
        SaveReceipt {
            database_options_change: None,
            ..settings
        },
        SaveReceipt {
            schema_change: Some(SaveSchemaChange::ColumnAdded),
            ..settings
        },
        SaveReceipt {
            query_change: Some(SaveQueryChange::ExpressionSaved),
            ..settings
        },
        SaveReceipt {
            matched_rows: Some(0),
            ..settings
        },
        SaveReceipt {
            changed_rows: Some(0),
            ..settings
        },
    ];
    for receipt in invalid {
        assert_eq!(
            SaveOutcome::Saved { receipt }.database_options_reopen_facts(),
            Err(DatabaseOptionsOutcomeError::UnqualifiedReplacementReceipt)
        );
    }
}

#[test]
fn current_original_overrides_history_and_released_after_does_not_invent_completion() {
    let original = SaveOutcome::Released {
        current_outcome: Some(SaveKnownOutcome::OriginalVerified),
        last_completed: Some(receipt()),
    }
    .database_options_reopen_facts()
    .unwrap();
    assert!(!original.replacement);
    assert_eq!(original.completed, Some(receipt()));

    let replacement = SaveOutcome::Released {
        current_outcome: Some(SaveKnownOutcome::ReplacementVerified),
        last_completed: None,
    }
    .database_options_reopen_facts()
    .unwrap();
    assert!(replacement.replacement);
    assert_eq!(replacement.completed, None);
}
