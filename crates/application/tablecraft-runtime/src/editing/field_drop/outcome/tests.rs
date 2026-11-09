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
        schema_change: Some(SaveSchemaChange::ColumnDropped),
        query_change: None,
        database_options_change: None,
        retirement: None,
        cleanup: None,
    }
}

#[test]
fn dropped_column_facts_are_distinct_from_rename_add_query_options_and_rows() {
    let expected = receipt();
    let facts = SaveOutcome::Saved { receipt: expected }
        .field_drop_reopen_facts()
        .unwrap();
    assert!(facts.replacement);
    assert_eq!(facts.completed, Some(expected));
    assert!(
        SaveOutcome::Saved { receipt: expected }
            .field_addition_reopen_facts()
            .is_err()
    );
    for field in 0..6 {
        let mut wrong = expected;
        match field {
            0 => wrong.schema_change = Some(SaveSchemaChange::ColumnAdded),
            1 => wrong.query_change = Some(SaveQueryChange::ExpressionSaved),
            2 => {
                wrong.database_options_change =
                    Some(SaveDatabaseOptionsChange::NameAutoCorrectDisabled)
            }
            3 => wrong.matched_rows = Some(0),
            4 => wrong.changed_rows = Some(0),
            _ => wrong.schema_change = Some(SaveSchemaChange::ColumnRenamed),
        }
        assert_eq!(
            SaveOutcome::Saved { receipt: wrong }.field_drop_reopen_facts(),
            Err(FieldDropOutcomeError::UnqualifiedReplacementReceipt)
        );
    }
    let original = SaveOutcome::Released {
        current_outcome: Some(SaveKnownOutcome::RolledBack),
        last_completed: Some(expected),
    };
    assert!(!original.field_drop_reopen_facts().unwrap().replacement);
}
