//! Creation facts are bound to the actual reviewed witness's schema and width.
use super::CreatedTableReopen;
use crate::editing::{SaveCompletion, SaveOutcome, SaveReceipt, SaveSchemaChange, outcome_facts};

/// Acknowledged image facts, separate from complete expected-image verification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableCreationReopenFacts {
    pub replacement: bool,
    pub completed: Option<SaveReceipt>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TableCreationOutcomeError {
    #[error("table creation replacement receipt is unqualified")]
    UnqualifiedReplacementReceipt,
}

fn facts(
    outcome: SaveOutcome,
    expected: SaveSchemaChange,
    inserted_rows: Option<usize>,
) -> Result<TableCreationReopenFacts, TableCreationOutcomeError> {
    let qualified = |receipt: SaveReceipt| {
        outcome_facts::known_replacement(receipt.known)
            && receipt.schema_change == Some(expected)
            && receipt.query_change.is_none()
            && receipt.database_options_change.is_none()
            && receipt.matched_rows.is_none()
            && receipt.changed_rows
                == match receipt.completion {
                    SaveCompletion::Applied => inserted_rows,
                    SaveCompletion::Recovered | SaveCompletion::Confirmed => None,
                }
    };
    let facts = outcome_facts::reopen_facts(outcome, qualified)
        .map_err(|()| TableCreationOutcomeError::UnqualifiedReplacementReceipt)?;
    Ok(TableCreationReopenFacts {
        replacement: facts.replacement,
        completed: facts.completed,
    })
}

impl SaveOutcome {
    /// Classify creation only for the exact accepted witness's operation and field
    /// count. Current Original overrides history; Released AFTER may have no
    /// receipt. The same complete witness must still verify the reopened file.
    ///
    /// # Errors
    /// Rejects uncertain actors and Saved replacement receipts of another scope.
    pub fn table_creation_reopen_facts(
        self,
        witness: &CreatedTableReopen,
    ) -> Result<TableCreationReopenFacts, TableCreationOutcomeError> {
        facts(
            self,
            witness.expected_schema_change(),
            witness.inserted_rows(),
        )
    }
}

#[cfg(test)]
mod tests;
