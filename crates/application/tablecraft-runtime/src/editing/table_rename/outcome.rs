//! A rename receipt cannot be confused with row, query or other schema work.
use crate::editing::{SaveOutcome, SaveReceipt, SaveSchemaChange, outcome_facts};

/// Acknowledged image facts; complete witness validation still precedes publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableRenameReopenFacts {
    pub replacement: bool,
    pub completed: Option<SaveReceipt>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TableRenameOutcomeError {
    #[error("table rename replacement receipt is unqualified")]
    UnqualifiedReplacementReceipt,
}

fn rename_receipt(receipt: SaveReceipt) -> bool {
    outcome_facts::known_replacement(receipt.known)
        && receipt.schema_change == Some(SaveSchemaChange::TableRenamed)
        && receipt.query_change.is_none()
        && receipt.database_options_change.is_none()
        && receipt.matched_rows.is_none()
        && receipt.changed_rows.is_none()
}

impl SaveOutcome {
    /// Classify acknowledged rename facts without acquiring or publishing a source.
    /// Current Original overrides history; Released AFTER may lack a completed
    /// receipt. An actual accepted TableRenameReopen remains required for reopening.
    ///
    /// # Errors
    /// Rejects uncertain actors and Saved replacement receipts of another scope.
    pub fn table_rename_reopen_facts(
        self,
    ) -> Result<TableRenameReopenFacts, TableRenameOutcomeError> {
        let facts = outcome_facts::reopen_facts(self, rename_receipt)
            .map_err(|()| TableRenameOutcomeError::UnqualifiedReplacementReceipt)?;
        Ok(TableRenameReopenFacts {
            replacement: facts.replacement,
            completed: facts.completed,
        })
    }
}

#[cfg(test)]
mod tests;
