use super::*;
use crate::editing::{
    SaveCompletion, SaveDatabaseOptionsChange, SaveKnownOutcome, SaveQueryChange,
};

fn receipt(schema: SaveSchemaChange) -> SaveReceipt {
    SaveReceipt {
        completion: SaveCompletion::Applied,
        known: SaveKnownOutcome::Committed,
        matched_rows: None,
        changed_rows: None,
        schema_change: Some(schema),
        query_change: None,
        database_options_change: None,
        retirement: None,
        cleanup: None,
    }
}

#[test]
fn creation_kind_and_exact_field_count_are_part_of_the_receipt_scope() {
    for expected in [
        SaveSchemaChange::FirstTableCreated { column_count: 2 },
        SaveSchemaChange::TableAdded { column_count: 2 },
    ] {
        let original = receipt(expected);
        assert_eq!(
            facts(SaveOutcome::Saved { receipt: original }, expected)
                .unwrap()
                .completed,
            Some(original)
        );
        for case in 0..6 {
            let mut wrong = original;
            match case {
                0 => wrong.schema_change = Some(SaveSchemaChange::TableRenamed),
                1 => wrong.schema_change = Some(SaveSchemaChange::TableAdded { column_count: 3 }),
                2 => wrong.matched_rows = Some(0),
                3 => wrong.changed_rows = Some(0),
                4 => wrong.query_change = Some(SaveQueryChange::ExpressionSaved),
                _ => {
                    wrong.database_options_change =
                        Some(SaveDatabaseOptionsChange::NameAutoCorrectDisabled)
                }
            }
            assert!(facts(SaveOutcome::Saved { receipt: wrong }, expected).is_err());
        }
    }
}

#[test]
fn creation_release_preserves_current_image_and_historical_receipt_distinction() {
    let expected = SaveSchemaChange::TableAdded { column_count: 2 };
    let original = receipt(expected);
    for known in [
        SaveKnownOutcome::RolledBack,
        SaveKnownOutcome::OriginalVerified,
    ] {
        let classified = facts(
            SaveOutcome::Released {
                current_outcome: Some(known),
                last_completed: Some(original),
            },
            expected,
        )
        .unwrap();
        assert!(!classified.replacement);
        assert_eq!(classified.completed, Some(original));
    }
    let after = facts(
        SaveOutcome::Released {
            current_outcome: Some(SaveKnownOutcome::ReplacementVerified),
            last_completed: None,
        },
        expected,
    )
    .unwrap();
    assert!(after.replacement);
    assert!(after.completed.is_none());
    let unknown = facts(
        SaveOutcome::Released {
            current_outcome: None,
            last_completed: None,
        },
        expected,
    )
    .unwrap();
    assert!(!unknown.replacement);
    assert!(unknown.completed.is_none());
    assert!(
        facts(
            SaveOutcome::Uncertain {
                last_known_outcome: None,
                last_completed: Some(original)
            },
            expected
        )
        .is_err()
    );
}

#[test]
fn failed_creation_owner_cannot_publish_after_before_resource_resolution() {
    use crate::editing::{SaveFailure, SaveFailureKind, SaveSessionState};
    let expected = SaveSchemaChange::FirstTableCreated { column_count: 2 };
    let outcome = SaveOutcome::Failed(SaveFailure {
        kind: SaveFailureKind::Finalization,
        phase: None,
        state: Some(SaveSessionState::RetainedJournal),
        current_outcome: Some(SaveKnownOutcome::ReplacementVerified),
        last_completed: Some(receipt(expected)),
        can_recover: true,
        can_release: true,
    });
    let classified = facts(outcome, expected).unwrap();
    assert!(!classified.replacement);
    assert!(classified.completed.is_none());
}
