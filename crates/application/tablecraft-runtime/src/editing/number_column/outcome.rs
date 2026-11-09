//! ColumnAdded receipt facts; only an accepted column witness permits reopening.
use crate::editing::{SaveOutcome, SaveReceipt, SaveSchemaChange, outcome_facts};

/// Observed Number schema completion, never a query or native reopen authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NumberColumnReopenFacts {
    /// Current acknowledged image may match the accepted column replacement.
    pub replacement: bool,
    /// An actual ColumnAdded-only completed receipt, if one was reported.
    pub completed: Option<SaveReceipt>,
}

/// A reported replacement receipt belongs to another operation or record scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum NumberColumnOutcomeError {
    /// Known replacement facts conflict with the separate column-only scope.
    #[error("saved Number column replacement receipt is unqualified")]
    UnqualifiedReplacementReceipt,
}

fn column_receipt(receipt: SaveReceipt) -> bool {
    outcome_facts::known_replacement(receipt.known)
        && receipt.schema_change == Some(SaveSchemaChange::ColumnAdded)
        && receipt.query_change.is_none()
        && receipt.database_options_change.is_none()
        && receipt.matched_rows.is_none()
        && receipt.changed_rows.is_none()
}

impl SaveOutcome {
    /// Classify only ColumnAdded facts from this acknowledged outcome. Current
    /// original/rollback facts override history; failed owners still require
    /// recovery/release. Released AFTER may have no completed receipt. Every
    /// replacement still needs the SAME accepted Number capability's complete
    /// AddedColumnReopen witness before publishing the freshly opened catalog.
    pub fn number_column_reopen_facts(
        self,
    ) -> Result<NumberColumnReopenFacts, NumberColumnOutcomeError> {
        let facts = outcome_facts::reopen_facts(self, column_receipt)
            .map_err(|()| NumberColumnOutcomeError::UnqualifiedReplacementReceipt)?;
        Ok(NumberColumnReopenFacts {
            replacement: facts.replacement,
            completed: facts.completed,
        })
    }
}

#[cfg(test)]
mod tests;
