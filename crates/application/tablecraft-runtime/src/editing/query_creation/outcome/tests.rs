//! Existing-definition completion cannot stand in for a new catalog object.
use super::*;
use crate::editing::{SaveCompletion, SaveKnownOutcome, SaveSchemaChange};

fn receipt() -> SaveReceipt {
    SaveReceipt {
        completion: SaveCompletion::Applied,
        known: SaveKnownOutcome::Committed,
        query_change: Some(SaveQueryChange::Created),
        matched_rows: None,
        changed_rows: None,
        schema_change: None,
        database_options_change: None,
        retirement: None,
        cleanup: None,
    }
}

#[test]
fn creation_reopen_refuses_expression_schema_and_even_zero_row_receipts() {
    let good = receipt();
    assert!(
        SaveOutcome::Saved { receipt: good }
            .query_creation_reopen_facts()
            .unwrap()
            .replacement
    );
    assert!(
        SaveOutcome::Saved { receipt: good }
            .query_expression_reopen_facts()
            .is_err()
    );
    for axis in 0..4 {
        let mut wrong = good;
        match axis {
            0 => wrong.query_change = Some(SaveQueryChange::ExpressionSaved),
            1 => wrong.schema_change = Some(SaveSchemaChange::TableAdded { column_count: 1 }),
            2 => wrong.matched_rows = Some(0),
            _ => wrong.changed_rows = Some(0),
        }
        assert_eq!(
            SaveOutcome::Saved { receipt: wrong }.query_creation_reopen_facts(),
            Err(QueryCreationOutcomeError::UnqualifiedReplacementReceipt)
        );
    }
}
