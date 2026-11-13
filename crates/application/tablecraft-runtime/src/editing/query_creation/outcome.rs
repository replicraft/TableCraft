//! New-query receipts never borrow the existing-expression or row/schema axis.
use crate::editing::{SaveOutcome, SaveQueryChange, SaveReceipt, outcome_facts};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueryCreationReopenFacts {
    pub replacement: bool,
    pub completed: Option<SaveReceipt>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum QueryCreationOutcomeError {
    #[error("query creation replacement receipt is unqualified")]
    UnqualifiedReplacementReceipt,
}

fn created(receipt: SaveReceipt) -> bool {
    outcome_facts::known_replacement(receipt.known)
        && receipt.query_change == Some(SaveQueryChange::Created)
        && receipt.schema_change.is_none()
        && receipt.database_options_change.is_none()
        && receipt.matched_rows.is_none()
        && receipt.changed_rows.is_none()
}

impl SaveOutcome {
    /// Classify actual query-created completion; a matching accepted full-image
    /// witness and explicit resource retirement are still required for reopen.
    pub fn query_creation_reopen_facts(
        self,
    ) -> Result<QueryCreationReopenFacts, QueryCreationOutcomeError> {
        let facts = outcome_facts::reopen_facts(self, created)
            .map_err(|()| QueryCreationOutcomeError::UnqualifiedReplacementReceipt)?;
        Ok(QueryCreationReopenFacts {
            replacement: facts.replacement,
            completed: facts.completed,
        })
    }
}

#[cfg(test)]
mod tests;
