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
    TableRenamed,
    /// One qualified closed empty table was removed from the native catalog.
    TableDropped,
    ColumnAdded,
    /// One existing field changed true name while retaining native identity.
    ColumnRenamed,
    /// One qualified nonprimary field was removed without changing surviving identities.
    ColumnDropped,
    FirstTableCreated {
        column_count: usize,
    },
    TableAdded {
        column_count: usize,
    },
}

/// Acknowledged query-definition facts, separate from table schema and rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveQueryChange {
    ExpressionSaved,
}

/// Database settings facts, separate from query/schema changes and row counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveDatabaseOptionsChange {
    NameAutoCorrectDisabled,
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
    pub query_change: Option<SaveQueryChange>,
    pub database_options_change: Option<SaveDatabaseOptionsChange>,
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
    /// The accepted actor ended without a terminal native result. These optional
    /// facts are historical observations, never the current file or a retry grant.
    Uncertain {
        last_known_outcome: Option<SaveKnownOutcome>,
        last_completed: Option<SaveReceipt>,
    },
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

/// Why a save or retained-resource resolution request was not accepted.
/// This is a start refusal, not the outcome of an already accepted write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveStartFailure {
    /// The OS refused actor startup before any request could be accepted.
    ThreadStart(std::io::ErrorKind),
    /// The actor ended and accepts no further work. A new source owner must be
    /// established explicitly; this does not establish the current file state.
    WorkerStopped,
    /// An active request, terminal report or retained native owner blocks a start.
    Busy,
    /// The supplied review no longer belongs to the current document generation.
    StaleDocument,
    /// The requested operation does not match the current capability/session state.
    InvalidState,
    /// No further worker-local request identity can be issued.
    Exhausted,
    /// This target has no implementation of the requested native save service.
    UnsupportedPlatform,
}

/// A request refused before acceptance by the save actor.
///
/// Starts that consume a reviewed capability or source path retain that exact
/// payload for [`Self::into_reviewed`]. Recovery/release requests identify an
/// already retained session instead of transferring a payload, so their errors
/// return `None`. A returned payload is not revalidated and grants no automatic
/// retry or permission to infer the result of earlier native work.
pub struct SaveStartError<T = ReviewedEdit> {
    /// The reason the new request was not accepted.
    pub kind: SaveStartFailure,
    pub(super) reviewed: Option<T>,
}

impl<T> SaveStartError<T> {
    /// Recover the exact unconsumed capability or source path supplied to a start.
    ///
    /// Returns `None` for refused recovery/release requests, which supplied only
    /// an existing report identity. This does not resolve or release any retained
    /// native resources, and it does not make a stale review current again.
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
