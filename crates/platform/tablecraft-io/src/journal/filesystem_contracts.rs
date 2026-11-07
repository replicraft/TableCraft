//! Platform-neutral retained filesystem journal lifecycle contracts.
use super::RecoveryOutcome;
use crate::IoError;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BindingError {
    #[error(transparent)]
    Io(#[from] IoError),
    #[error("invalid pair binding record length")]
    InvalidLength,
    #[error("invalid pair binding record signature")]
    InvalidMagic,
    #[error("unsupported paired journal protocol")]
    UnsupportedProtocol,
    #[error("unsupported pair object identity scheme")]
    UnsupportedIdentityScheme,
    #[error("pair binding checksum mismatch")]
    ChecksumMismatch,
    #[error("pair storage roles alias the same object")]
    AliasedRoles,
    #[error("pair binding does not match actual retained objects")]
    ObjectMismatch,
    #[error("paired intent length mismatch")]
    IntentLengthMismatch,
    #[error("paired intent digest mismatch")]
    IntentDigestMismatch,
    #[error("paired journal offset is invalid")]
    InvalidOffset,
    #[error("pair binding flags or reserved bytes are invalid")]
    InvalidFlags,
    #[error("paired intent exceeds its configured byte ceiling")]
    IntentLimit,
}

/// Sanitized filesystem pairing errors, never a claim that an edit rolled back.
#[derive(Debug, Error)]
pub enum PairError {
    /// Storage, cancellation, allocation or resource failure.
    #[error(transparent)]
    Io(#[from] IoError),
    /// Persisted data framing or object association failed.
    #[error(transparent)]
    Binding(#[from] BindingError),
    /// Selected actual resources differ from the binding or alias roles.
    #[error("journal bundle object identity mismatch")]
    IdentityMismatch,
    /// Opaque intent bytes differ from their binding.
    #[error("journal bundle intent mismatch")]
    IntentMismatch,
    /// The caller's explicit intent-read ceiling was exceeded before allocation.
    #[error("journal bundle intent byte ceiling exceeded")]
    IntentLimit,
    /// A live operation requires an acknowledged exclusive lease/valid phase.
    #[error("journal pair is not in the required lifecycle state")]
    InvalidState,
    /// An active name already equals its reserved retirement name.
    #[error("journal bundle uses its reserved retirement name; diagnosis required")]
    ReservedName,
}

/// A phase and known outcome are separate: release errors cannot erase completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PairState {
    /// An intact reserved retired bundle requires write-free native confirmation.
    RetiredVerificationRequired,
    /// New durable bundle is initialized; no native storage borrow occurred.
    Ready,
    /// An active entry check/known collision prevented retirement.
    RetirementBlocked,
    /// Rename callback outcome is unknown; diagnosis must not replay it.
    RetirementUncertain,
    /// Rename succeeded; only the held-parent barrier may be explicitly retried.
    RetirementUnconfirmed,
    /// Intact bundle retirement is durably acknowledged.
    Retired,
    /// Retirement is durable, but garbage cleanup may have partly taken effect.
    RetiredGarbageUncertain,
    /// Retired garbage cleanup acknowledged completion.
    RetiredClean,
    /// Normal access is excluded until explicit recovery/resolution.
    RecoveryRequired,
    /// Caller acknowledged native/generic completion; durable retirement is pending.
    OutcomeRecorded,
    /// An unlock may have taken effect; no storage access remains available.
    ReleaseUncertain,
    /// Handles acknowledged release; any retained bundle still needs lifecycle work.
    Released,
}

/// Acknowledged lock state of this retained target, not native write permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetLockState {
    /// The retained descriptor acknowledged the exclusive lock acquisition.
    Held,
    /// Unlock returned an error; the descriptor survives, but exclusivity is unknown.
    ReleaseUncertain,
    /// Unlock acknowledged completion; no further read/storage access is offered.
    Released,
}

/// Namespace stage that failed after a separately known native outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinalizationPhase {
    /// The active bundle was not safely retired or its outcome is unknown.
    Retirement,
    /// Rename succeeded, but its parent barrier was not acknowledged.
    RetirementBarrier,
    /// Durable retirement succeeded; only retired garbage may remain.
    GarbageCleanup,
    /// Caller attempted an operation outside its retained phase/permit.
    State,
}

/// Completion facts survive namespace errors; no variant authorizes replay.
#[derive(Debug, Error)]
#[error("journal finalization failed ({phase:?}); known outcome {outcome:?}: {cause}")]
pub struct FinalizationError {
    /// Failed lifecycle stage, without paths or native values.
    pub phase: FinalizationPhase,
    /// Acknowledged native result, independent of cleanup/barrier failure.
    pub outcome: Option<RecoveryOutcome>,
    /// Sanitized failure; retain the owned pair for explicit diagnosis.
    #[source]
    pub cause: PairError,
}

/// Namespace cleanup supported by a retained platform owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetiredCleanupPolicy {
    /// Remove verified retired entries and acknowledge the captured-parent barrier.
    RemoveWithBarrier,
    /// Preserve the complete retired artifact for explicit later disposal.
    RetainIntact,
}

/// Acknowledged namespace retirement mechanism, not a native write permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetirementGuarantee {
    /// Retirement rename and the retained parent-directory barrier acknowledged.
    CapturedParentBarrier,
    /// NTFS acknowledged the qualified write-through retirement rename.
    NtfsWriteThroughRename,
}

/// Observed terminal artifact disposition, independent of the native outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CleanupDisposition {
    /// Verified retired entries were removed with their required barrier.
    Removed,
    /// The complete retired artifact intentionally remains intact.
    RetainedArtifact,
}
