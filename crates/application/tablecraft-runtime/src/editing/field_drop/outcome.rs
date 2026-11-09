//! ColumnDropped receipt facts; only an accepted column witness permits reopening.
use crate::editing::{SaveOutcome, SaveReceipt, SaveSchemaChange, outcome_facts};

/// Observed dropped-field schema completion, never a query or native reopen authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldDropReopenFacts {
    /// Current acknowledged image may match the accepted column replacement.
    pub replacement: bool,
    /// An actual ColumnDropped-only completed receipt, if one was reported.
    pub completed: Option<SaveReceipt>,
}

/// A reported replacement receipt belongs to another operation or record scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum FieldDropOutcomeError {
    /// Known replacement facts conflict with the separate column-only scope.
    #[error("saved-field DROP replacement receipt is unqualified")]
    UnqualifiedReplacementReceipt,
}

fn column_receipt(receipt: SaveReceipt) -> bool {
    outcome_facts::known_replacement(receipt.known)
        && receipt.schema_change == Some(SaveSchemaChange::ColumnDropped)
        && receipt.query_change.is_none()
        && receipt.database_options_change.is_none()
        && receipt.matched_rows.is_none()
        && receipt.changed_rows.is_none()
}

impl SaveOutcome {
    /// Classify only ColumnDropped facts from this acknowledged outcome. Current
    /// original/rollback facts override history; failed owners still require
    /// recovery/release. Released AFTER may have no completed receipt. Every
    /// replacement still needs the SAME accepted field capability's complete
    /// FieldDroppedReopen witness before publishing the freshly opened catalog.
    pub fn field_drop_reopen_facts(self) -> Result<FieldDropReopenFacts, FieldDropOutcomeError> {
        let facts = outcome_facts::reopen_facts(self, column_receipt)
            .map_err(|()| FieldDropOutcomeError::UnqualifiedReplacementReceipt)?;
        Ok(FieldDropReopenFacts {
            replacement: facts.replacement,
            completed: facts.completed,
        })
    }
}
#[cfg(test)]
mod tests;
