//! Nonreplaceable save outcomes. Presentation generations cannot suppress them.
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod native;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use native::SaveWorker;

use super::ReviewedEdit;
use std::fmt;
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
use std::path::PathBuf;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaveReceipt {
    pub completion: SaveCompletion,
    pub known: SaveKnownOutcome,
    pub matched_rows: Option<usize>,
    pub changed_rows: Option<usize>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveStartFailure {
    Busy,
    StaleDocument,
    InvalidState,
    Exhausted,
    UnsupportedPlatform,
}

/// A rejected start returns its unconsumed capability to the caller.
pub struct SaveStartError {
    pub kind: SaveStartFailure,
    pub(super) reviewed: Option<ReviewedEdit>,
}

impl SaveStartError {
    pub fn into_reviewed(self) -> Option<ReviewedEdit> {
        self.reviewed
    }
}

impl fmt::Debug for SaveStartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SaveStartError")
            .field("kind", &self.kind)
            .finish()
    }
}

impl fmt::Display for SaveStartError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "save request was not accepted ({:?})", self.kind)
    }
}

impl std::error::Error for SaveStartError {}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub struct SaveWorker;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
impl SaveWorker {
    pub fn new(_repaint: impl Fn() + Send + Sync + 'static) -> Self {
        Self
    }
    pub fn start(&self, reviewed: ReviewedEdit, _bundle: PathBuf) -> Result<u64, SaveStartError> {
        Err(SaveStartError {
            kind: SaveStartFailure::UnsupportedPlatform,
            reviewed: Some(reviewed),
        })
    }
    pub fn poll(&self) -> Option<SaveReport> {
        None
    }
    pub fn cancel(&self, _id: u64) -> bool {
        false
    }
    pub fn acknowledge(&self, _id: u64) -> bool {
        false
    }
    pub fn recover(&self, _id: u64) -> Result<u64, SaveStartError> {
        Err(SaveStartError {
            kind: SaveStartFailure::UnsupportedPlatform,
            reviewed: None,
        })
    }
    pub fn release(&self, _id: u64) -> Result<u64, SaveStartError> {
        Err(SaveStartError {
            kind: SaveStartFailure::UnsupportedPlatform,
            reviewed: None,
        })
    }
    pub fn is_busy(&self) -> bool {
        false
    }
}
