//! Nonreplaceable save outcomes. Presentation generations cannot suppress them.
use super::ReviewedEdit;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveOperation {
    Apply,
    Recover,
    Release,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SavePhase {
    Planning,
    Intent,
    Setup,
    Prepare,
    Commit,
    Recovery,
    Confirmation,
    Refresh,
    Finalization,
    Release,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveSessionState {
    Ready,
    RefreshRequired,
    RetainedJournal,
    IncompleteSetup,
    ReleaseUncertain,
    Released,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveFailureKind {
    StaleDocument,
    Open,
    SourceChanged,
    Cancelled,
    Resource,
    Planning,
    Intent,
    Setup,
    Prepare,
    Commit,
    Recovery,
    Finalization,
    Release,
    InvalidState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveKnownOutcome {
    Committed,
    RolledBack,
    OriginalVerified,
    ReplacementVerified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveCompletion {
    Applied,
    Recovered,
    Confirmed,
}

/// Acknowledged schema facts are separate from affected-record counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveSchemaChange {
    FirstTableCreated { column_count: usize },
    TableAdded { column_count: usize },
}

/// Acknowledged namespace retirement, separate from the native data result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveRetirementGuarantee {
    CapturedParentBarrier,
    NtfsWriteThroughRename,
}

/// What actually happened to the retired recovery artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveCleanupDisposition {
    Removed,
    RetainedArtifact,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaveReceipt {
    pub completion: SaveCompletion,
    pub known: SaveKnownOutcome,
    pub matched_rows: Option<usize>,
    pub changed_rows: Option<usize>,
    pub schema_change: Option<SaveSchemaChange>,
    pub retirement: Option<SaveRetirementGuarantee>,
    pub cleanup: Option<SaveCleanupDisposition>,
}

/// These are observed resource facts, not an inferred rollback or a retry grant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaveFailure {
    pub kind: SaveFailureKind,
    pub phase: Option<SavePhase>,
    pub state: Option<SaveSessionState>,
    pub current_outcome: Option<SaveKnownOutcome>,
    pub last_completed: Option<SaveReceipt>,
    pub can_recover: bool,
    pub can_release: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveOutcome {
    Saved {
        receipt: SaveReceipt,
    },
    Released {
        current_outcome: Option<SaveKnownOutcome>,
        last_completed: Option<SaveReceipt>,
    },
    Failed(SaveFailure),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaveReport {
    pub id: u64,
    pub operation: SaveOperation,
    pub outcome: SaveOutcome,
}

impl SaveReport {
    /// The original apply stopped before journal setup and target writes.
    /// This says nothing about unrelated writers or a later recovery operation.
    pub fn stopped_before_target_writes(self) -> bool {
        if self.operation != SaveOperation::Apply {
            return false;
        }
        let SaveOutcome::Failed(failure) = self.outcome else {
            return false;
        };
        failure.state.is_none()
            || (matches!(failure.phase, Some(SavePhase::Planning | SavePhase::Intent))
                && matches!(
                    failure.state,
                    Some(SaveSessionState::Ready | SaveSessionState::RefreshRequired)
                ))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveStartFailure {
    Busy,
    StaleDocument,
    InvalidState,
    Exhausted,
    UnsupportedPlatform,
}

/// A rejected start returns its unconsumed capability to the caller.
pub struct SaveStartError<T = ReviewedEdit> {
    pub kind: SaveStartFailure,
    pub(super) reviewed: Option<T>,
}

impl<T> SaveStartError<T> {
    pub fn into_reviewed(self) -> Option<T> {
        self.reviewed
    }
}

impl<T> fmt::Debug for SaveStartError<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SaveStartError")
            .field("kind", &self.kind)
            .finish()
    }
}

impl<T> fmt::Display for SaveStartError<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "save request was not accepted ({:?})", self.kind)
    }
}

impl<T> std::error::Error for SaveStartError<T> {}
