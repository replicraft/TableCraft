//! Settings-only completion facts; an accepted full-image witness is still required.
use crate::editing::{SaveDatabaseOptionsChange, SaveOutcome, SaveReceipt, outcome_facts};

/// Acknowledged database-settings observations, never permission to mutate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DatabaseOptionsReopenFacts {
    pub replacement: bool,
    pub completed: Option<SaveReceipt>,
}

/// A known replacement receipt belongs to a different operation scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DatabaseOptionsOutcomeError {
    #[error("saved database settings replacement receipt is unqualified")]
    UnqualifiedReplacementReceipt,
}

fn options_receipt(receipt: SaveReceipt) -> bool {
    outcome_facts::known_replacement(receipt.known)
        && receipt.database_options_change
            == Some(SaveDatabaseOptionsChange::NameAutoCorrectDisabled)
        && receipt.schema_change.is_none()
        && receipt.query_change.is_none()
        && receipt.matched_rows.is_none()
        && receipt.changed_rows.is_none()
}

impl SaveOutcome {
    /// Read only the settings scope of this exact acknowledged outcome. Current
    /// original/rollback observations override older completion. Failed owners
    /// must recover or release; a released AFTER need not have a completed receipt.
    /// Reopening still requires the same accepted capability's complete witness.
    pub fn database_options_reopen_facts(
        self,
    ) -> Result<DatabaseOptionsReopenFacts, DatabaseOptionsOutcomeError> {
        let facts = outcome_facts::reopen_facts(self, options_receipt)
            .map_err(|()| DatabaseOptionsOutcomeError::UnqualifiedReplacementReceipt)?;
        Ok(DatabaseOptionsReopenFacts {
            replacement: facts.replacement,
            completed: facts.completed,
        })
    }
}

#[cfg(test)]
mod tests;
