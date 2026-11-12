//! Schema-only receipts remain separate from native retirement metadata.
use crate::editing::{SaveOutcome, SaveReceipt, SaveSchemaChange, outcome_facts};

/// Terminal source-image facts; complete expected-image verification is separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexDropReopenFacts {
    /// The terminal outcome acknowledges the independently verified replacement.
    pub replacement: bool,
    /// Actual completed receipt, when available; never inferred from equal bytes.
    pub completed: Option<SaveReceipt>,
}

/// A replacement receipt describes another operation or data-row effects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum IndexDropOutcomeError {
    #[error("index drop replacement receipt is unqualified")]
    UnqualifiedReplacementReceipt,
}

impl SaveOutcome {
    /// Classify an acknowledged schema-only index result. Source population is
    /// never reported as changed/matched data. Exact AFTER must still be opened.
    pub fn index_drop_reopen_facts(self) -> Result<IndexDropReopenFacts, IndexDropOutcomeError> {
        let facts = outcome_facts::reopen_facts(self, |receipt| {
            outcome_facts::known_replacement(receipt.known)
                && receipt.schema_change == Some(SaveSchemaChange::IndexDropped)
                && receipt.matched_rows.is_none()
                && receipt.changed_rows.is_none()
                && receipt.query_change.is_none()
                && receipt.database_options_change.is_none()
        })
        .map_err(|()| IndexDropOutcomeError::UnqualifiedReplacementReceipt)?;

        Ok(IndexDropReopenFacts {
            replacement: facts.replacement,
            completed: facts.completed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editing::{SaveCompletion, SaveKnownOutcome};

    #[test]
    fn index_receipts_reject_affected_rows_or_another_schema_effect() {
        let original = SaveReceipt {
            completion: SaveCompletion::Applied,
            known: SaveKnownOutcome::Committed,
            matched_rows: None,
            changed_rows: None,
            schema_change: Some(SaveSchemaChange::IndexDropped),
            query_change: None,
            database_options_change: None,
            retirement: None,
            cleanup: None,
        };
        assert!(
            SaveOutcome::Saved { receipt: original }
                .index_drop_reopen_facts()
                .unwrap()
                .replacement
        );
        for case in 0..3 {
            let mut wrong = original;
            match case {
                0 => wrong.matched_rows = Some(6),
                1 => wrong.changed_rows = Some(6),
                _ => wrong.schema_change = Some(SaveSchemaChange::TableAdded { column_count: 1 }),
            }
            assert!(
                SaveOutcome::Saved { receipt: wrong }
                    .index_drop_reopen_facts()
                    .is_err()
            );
        }
        let mut original_image = original;
        original_image.known = SaveKnownOutcome::RolledBack;
        assert!(
            !SaveOutcome::Saved {
                receipt: original_image
            }
            .index_drop_reopen_facts()
            .unwrap()
            .replacement
        );
        assert!(
            SaveOutcome::Uncertain {
                last_known_outcome: Some(SaveKnownOutcome::Committed),
                last_completed: Some(original),
            }
            .index_drop_reopen_facts()
            .is_err()
        );
    }
}
