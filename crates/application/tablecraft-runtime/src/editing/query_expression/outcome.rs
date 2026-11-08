//! Query-definition receipt facts; a caller still needs its accepted AFTER witness.
use crate::editing::{SaveKnownOutcome, SaveOutcome, SaveQueryChange, SaveReceipt};

/// Observed query-only completion facts, never permission to reopen or replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QueryExpressionReopenFacts {
    /// Current acknowledged image may match the accepted query replacement.
    pub replacement: bool,
    /// An actual query-only completed receipt, if one was reported.
    pub completed: Option<SaveReceipt>,
}

/// A reported replacement receipt belongs to another operation or row scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum QueryExpressionOutcomeError {
    #[error("saved query replacement receipt is unqualified")]
    /// Known replacement facts conflict with the separate query-only scope.
    UnqualifiedReplacementReceipt,
}

fn known_replacement(known: SaveKnownOutcome) -> bool {
    matches!(
        known,
        SaveKnownOutcome::Committed | SaveKnownOutcome::ReplacementVerified
    )
}

fn query_receipt(receipt: SaveReceipt) -> bool {
    known_replacement(receipt.known)
        && receipt.query_change == Some(SaveQueryChange::ExpressionSaved)
        && receipt.schema_change.is_none()
        && receipt.database_options_change.is_none()
        && receipt.matched_rows.is_none()
        && receipt.changed_rows.is_none()
}

impl SaveOutcome {
    /// Classify only query-definition facts from this acknowledged outcome.
    /// Current original/rollback facts override an older completed receipt.
    /// Failed owners require explicit recovery/release, even with known AFTER.
    /// A released replacement may lack a finalization receipt; no receipt is
    /// manufactured. Every replacement still requires the SAME accepted native
    /// capability's full-image witness before a fresh catalog can be published.
    pub fn query_expression_reopen_facts(
        self,
    ) -> Result<QueryExpressionReopenFacts, QueryExpressionOutcomeError> {
        let (replacement, completed) = match self {
            Self::Saved { receipt } => {
                if known_replacement(receipt.known) && !query_receipt(receipt) {
                    return Err(QueryExpressionOutcomeError::UnqualifiedReplacementReceipt);
                }
                let replacement = query_receipt(receipt);
                (replacement, replacement.then_some(receipt))
            }
            Self::Released {
                current_outcome,
                last_completed,
            } => {
                let completed = last_completed.filter(|receipt| query_receipt(*receipt));
                let replacement = current_outcome.map_or(completed.is_some(), known_replacement);
                (replacement, completed)
            }
            Self::Failed(_) => (false, None),
        };
        Ok(QueryExpressionReopenFacts {
            replacement,
            completed,
        })
    }
}

#[cfg(test)]
mod tests;
