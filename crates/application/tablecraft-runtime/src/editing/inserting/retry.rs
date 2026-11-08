//! Read-only original-content evidence for an explicit insertion retry workflow.
use super::InsertTarget;
use crate::{SaveKnownOutcome, SaveOperation, SaveOutcome, SaveReport};
use bytes::Bytes;
use std::path::{Path, PathBuf};

/// Exact original INSERT source, without rollback, replay or save permission.
/// Hosts must establish the previous operation's outcome separately. Reopening
/// this witness never begins insertion or reuses the old consuming review.
pub struct InsertRetryWitness {
    pub(in crate::editing) path: PathBuf,
    pub(in crate::editing) original: Bytes,
    pub(in crate::editing) target: InsertTarget,
}

impl InsertRetryWitness {
    pub fn source_path(&self) -> &Path {
        &self.path
    }

    /// Original target metadata. Its revision is historical after reopening.
    pub fn target(&self) -> &InsertTarget {
        &self.target
    }

    pub(crate) fn expected_bytes(&self) -> &[u8] {
        &self.original
    }
}

/// Conservative display eligibility for one host-bound INSERT save chain.
/// The host supplies only that chain's matching reports, including resolution IDs.
/// This grants neither retry nor rollback authority: reopening still checks every
/// original byte, and insertion still requires a fresh consuming review.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InsertRetryEvidence {
    apply_id: Option<u64>,
    no_writes: bool,
    original_known: bool,
    committed_observed: bool,
}

impl InsertRetryEvidence {
    pub fn observe(&mut self, report: SaveReport) {
        if report.operation == SaveOperation::Apply {
            if self.apply_id != Some(report.id) {
                *self = Self {
                    apply_id: Some(report.id),
                    ..Self::default()
                };
            }
            self.no_writes |= report.stopped_before_target_writes();
        }
        let current = match report.outcome {
            SaveOutcome::Saved { receipt } => Some(receipt.known),
            SaveOutcome::Failed(failure) => failure.current_outcome,
            SaveOutcome::Released {
                current_outcome, ..
            } => current_outcome,
        };
        match current {
            Some(SaveKnownOutcome::Committed | SaveKnownOutcome::ReplacementVerified) => {
                self.committed_observed = true;
                self.original_known = false;
            }
            Some(SaveKnownOutcome::RolledBack | SaveKnownOutcome::OriginalVerified) => {
                self.original_known = true;
            }
            None => {}
        }
    }

    pub fn can_retry_original(&self) -> bool {
        !self.committed_observed && (self.no_writes || self.original_known)
    }
}

#[cfg(test)]
mod tests;
