//! Only exact table-schema receipts may pair with an accepted DROP reopen witness.
use crate::editing::{SaveOutcome, SaveReceipt, SaveSchemaChange, outcome_facts};

/// Observed persistence facts, separate from authority to publish a fresh catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TableDropReopenFacts {
    pub replacement: bool,
    pub completed: Option<SaveReceipt>,
}

/// A claimed replacement receipt belongs to another operation or row scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TableDropOutcomeError {
    #[error("table DROP replacement receipt is unqualified")]
    UnqualifiedReplacementReceipt,
}

fn table_receipt(receipt: SaveReceipt) -> bool {
    outcome_facts::known_replacement(receipt.known)
        && receipt.schema_change == Some(SaveSchemaChange::TableDropped)
        && receipt.query_change.is_none()
        && receipt.database_options_change.is_none()
        && receipt.matched_rows.is_none()
        && receipt.changed_rows.is_none()
}

impl SaveOutcome {
    /// Classify acknowledged table-only outcomes without inventing rollback or
    /// publishing data. Every replacement still needs the same accepted witness.
    ///
    /// # Errors
    /// Rejects a replacement receipt carrying another schema/query/record scope.
    pub fn table_drop_reopen_facts(self) -> Result<TableDropReopenFacts, TableDropOutcomeError> {
        let facts = outcome_facts::reopen_facts(self, table_receipt)
            .map_err(|()| TableDropOutcomeError::UnqualifiedReplacementReceipt)?;
        Ok(TableDropReopenFacts {
            replacement: facts.replacement,
            completed: facts.completed,
        })
    }
}
