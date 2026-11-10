use super::{KnownOutcome, OfflineWriteSession, Receipt, SessionState};
use crate::editing::{NativeEditError, journal, sql};
use std::{error::Error, fmt};
#[cfg(feature = "ace16-create")]
use tablecraft_ace::{
    AddColumnIntentError, AddTableError, AddTableIntentError, DatabaseOptionsIntentError,
    DefaultZeroAddColumnIntentError, FieldAdditionAfterOffHistoryIntentError,
    FieldAdditionAfterOffIntentError, FieldAdditionIntentError, FieldDropIntentError,
    FieldHistoryOptionsIntentError, FieldRenameIntentError, FirstTableError, FirstTableIntentError,
    RepeatedDefaultZeroAddColumnIntentError,
};
use tablecraft_ace::{
    CurrentFieldAdditionIntentError, InspectError, NativeIntentError,
    NativeRecoveryQualificationError, QueryExpressionEditsIntentError, QueryExpressionIntentError,
    QueryExpressionRecoveryError, TableDropIntentError, TableRenameIntentError,
};
use tablecraft_io::{
    IoError,
    journal::{
        CommitError, JournalError,
        filesystem::{FinalizationError, PairError, PairSetupFailure, PairState},
    },
};
use thiserror::Error;

/// Failure before a session acquires/publishes its retained target owner.
#[derive(Debug, Error)]
pub enum OpenError {
    /// Resource, cancellation or retained-slot allocation failed before open.
    #[error(transparent)]
    Native(#[from] InspectError),
    /// Existing target capture or exclusive locking failed.
    #[error(transparent)]
    Io(#[from] IoError),
}

/// Operation phase at failure; it does not imply rollback or absent output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Normal capture, SQL binding or native planning.
    Planning,
    /// Before-bound intent encoding before journal creation.
    Intent,
    /// Durable bundle creation or explicit existing-bundle association.
    Setup,
    /// Expected-preimage journal preparation; target writes have not begun.
    Prepare,
    /// Commit attempt; every error requires explicit recovery.
    Commit,
    /// Explicit qualification/recovery of a retained active journal.
    Recovery,
    /// Write-free qualification of an intact retired journal.
    Confirmation,
    /// Explicit new complete capture after a failed read.
    Refresh,
    /// Retirement, cleanup or retained-target transfer after known completion.
    Finalization,
    /// Explicit lock release; an error can invalidate the held-lease claim.
    Release,
}

/// Sanitized typed cause. Resources remain in Failure rather than being discarded.
#[derive(Debug, Error)]
pub enum Cause {
    /// Original table DROP request or complete native reconstruction refused.
    #[error(transparent)]
    TableDropIntent(TableDropIntentError),
    /// Explicit option intent, source admission or complete reconstruction failed.
    #[cfg(feature = "ace16-create")]
    #[error(transparent)]
    DatabaseOptionsIntent(DatabaseOptionsIntentError),
    /// OFF-after-field-history original request or complete reconstruction failed.
    #[cfg(feature = "ace16-create")]
    #[error(transparent)]
    FieldHistoryDatabaseOptionsIntent(FieldHistoryOptionsIntentError),
    /// ADD-column framing, source qualification or exact reconstruction failed.
    #[cfg(feature = "ace16-create")]
    #[error(transparent)]
    AddColumnIntent(AddColumnIntentError),
    /// Explicit-zero declaration, source or resizable reconstruction failed.
    #[cfg(feature = "ace16-create")]
    #[error(transparent)]
    DefaultZeroAddColumnIntent(DefaultZeroAddColumnIntentError),
    /// Distinct repeated explicit-zero original request qualification.
    #[cfg(feature = "ace16-create")]
    #[error(transparent)]
    RepeatedDefaultZeroAddColumnIntent(RepeatedDefaultZeroAddColumnIntentError),
    /// Typed field declaration reconstruction or native original-intent refusal.
    #[cfg(feature = "ace16-create")]
    #[error(transparent)]
    FieldAdditionIntent(FieldAdditionIntentError),
    /// Original ADD-after-OFF request or fresh complete reconstruction refused.
    #[cfg(feature = "ace16-create")]
    #[error(transparent)]
    FieldAdditionAfterOffIntent(FieldAdditionAfterOffIntentError),
    /// Original repeated post-OFF field request or full native reconstruction refused.
    #[cfg(feature = "ace16-create")]
    #[error(transparent)]
    FieldAdditionAfterOffHistoryIntent(FieldAdditionAfterOffHistoryIntentError),
    /// Current metadata ADD request or fresh native reconstruction refused.
    #[error(transparent)]
    CurrentFieldAdditionIntent(CurrentFieldAdditionIntentError),
    /// Original field-rename request or complete native reconstruction refused.
    #[cfg(feature = "ace16-create")]
    #[error(transparent)]
    FieldRenameIntent(FieldRenameIntentError),
    /// Original saved-field DROP request or complete native reconstruction refused.
    #[cfg(feature = "ace16-create")]
    #[error(transparent)]
    FieldDropIntent(FieldDropIntentError),
    /// This retained capability does not permit the requested transition.
    #[error("offline session state does not permit this operation")]
    InvalidState,
    /// The retained target differs from the complete image reviewed by the host.
    #[error("the database changed after it was viewed")]
    SourceChanged,
    /// Native capture, allocation, cancellation or resource failure.
    #[error(transparent)]
    Native(InspectError),
    /// Capturing the typed table snapshot or validating its declaration failed.
    #[error(transparent)]
    Editor(NativeEditError),
    /// Snapshot-local typed handle conversion or normal native planning refused.
    #[error(transparent)]
    Editing(journal::Error),
    /// Closed shared SQL or normal native planning refused the request.
    #[error(transparent)]
    Planning(sql::RecoverableError),
    /// Local DELETE shape, predicate evaluation or normal native planning refused.
    #[error(transparent)]
    DeletePlanning(sql::RecoverableDeleteError),
    /// Closed local INSERT binding or normal native planning refused.
    #[error(transparent)]
    InsertPlanning(sql::RecoverableInsertError),
    /// Source-bound first-table construction refused before journal preparation.
    #[cfg(feature = "ace16-create")]
    #[error(transparent)]
    FirstTablePlanning(FirstTableError),
    /// First-table intent framing or encoding failed without another decoder retry.
    #[cfg(feature = "ace16-create")]
    #[error(transparent)]
    FirstTableIntent(FirstTableIntentError),
    /// Source-bound table addition refused before journal preparation.
    #[cfg(feature = "ace16-create")]
    #[error(transparent)]
    AddTablePlanning(AddTableError),
    /// Add-table intent framing or encoding failed without decoder fallback.
    #[cfg(feature = "ace16-create")]
    #[error(transparent)]
    AddTableIntent(AddTableIntentError),
    /// Closed existing-query request encoding failed without another decoder retry.
    #[error(transparent)]
    QueryExpressionIntent(QueryExpressionIntentError),
    /// Fresh original query definition, identity or complete image did not qualify.
    #[error(transparent)]
    QueryExpressionQualification(QueryExpressionRecoveryError),
    /// TCQE0002 encoding, fresh source, fragment or complete-image proof refused.
    #[error(transparent)]
    QueryExpressionEditsIntent(QueryExpressionEditsIntentError),
    /// Table-rename intent framing or encoding failed without decoder fallback.
    #[error(transparent)]
    TableRenameIntent(TableRenameIntentError),
    /// Original typed intent framing/binding or encoding failed.
    #[error(transparent)]
    Intent(NativeIntentError),
    /// Target resource or release failure, retaining its portable cause.
    #[error(transparent)]
    Io(IoError),
    /// Bundle setup failed; inspect Failure::setup_failure for retained details.
    #[error("journal setup failed; retained setup details remain available")]
    Setup,
    /// The retained pair does not permit the requested storage borrow.
    #[error(transparent)]
    Pair(PairError),
    /// Journal preparation failed; retain the exact target/journal for diagnosis.
    #[error(transparent)]
    Prepare(JournalError),
    /// A commit attempt failed and requires explicit qualified recovery.
    #[error(transparent)]
    Commit(CommitError),
    /// Normal native replanning did not qualify a reviewed plan or checked journal.
    #[error(transparent)]
    Qualification(NativeRecoveryQualificationError),
    /// Checked journal inspection, recovery or confirmation failed.
    #[error(transparent)]
    Recovery(JournalError),
    /// A known native/current-image result survived a terminal namespace failure.
    #[error(transparent)]
    Finalization(FinalizationError),
    /// The cleaned pair could not transfer its still-held target.
    #[error(transparent)]
    Transfer(PairError),
    /// Incomplete setup lock release failed; original setup diagnostics survive.
    #[error(transparent)]
    ReleaseSetup(PairError),
    /// Pair lock release failed; no implicit retry follows.
    #[error(transparent)]
    ReleasePair(PairError),
}

/// Owns exact retained resources; returning an error performs no unlock or retry.
pub struct Failure {
    session: OfflineWriteSession,
    phase: Phase,
    cause: Cause,
}

impl Failure {
    pub(super) fn new(session: OfflineWriteSession, phase: Phase, cause: Cause) -> Self {
        Self {
            session,
            phase,
            cause,
        }
    }

    /// Typed sanitized cause; setup capabilities are separately retained.
    pub fn cause(&self) -> &Cause {
        &self.cause
    }

    /// Phase that failed, without inferring an operation outcome.
    pub const fn phase(&self) -> Phase {
        self.phase
    }

    /// Current session capability state after the failure.
    pub fn state(&self) -> SessionState {
        self.session.state()
    }

    /// Known fact for this attempted operation, distinct from a previous receipt.
    pub fn current_outcome(&self) -> Option<KnownOutcome> {
        self.session.current_outcome()
    }

    /// Earlier completion retained even when the present action failed.
    pub fn last_completed(&self) -> Option<Receipt> {
        self.session.last_completed()
    }

    /// Retained setup cause and resource owners; no namespace is auto-cleaned.
    pub fn setup_failure(&self) -> Option<&PairSetupFailure> {
        self.session.setup_failure()
    }

    /// Exact retained pair capability/release state, when a pair exists.
    pub fn pair_state(&self) -> Option<PairState> {
        self.session.pair_state()
    }

    /// Retained namespace state independently of a later lock-release failure.
    pub fn pair_namespace_state(&self) -> Option<PairState> {
        self.session.pair_namespace_state()
    }

    /// Recover the SAME resource owner; blocked/uncertain states stay blocked.
    pub fn into_session(self) -> OfflineWriteSession {
        self.session
    }

    /// Classify pre-journal planning/encoding refusals for executable boundaries.
    pub fn invalid_input(&self) -> bool {
        matches!(self.phase, Phase::Planning | Phase::Intent)
            && matches!(
                self.state(),
                SessionState::Ready | SessionState::RefreshRequired
            )
            && self.pair_state().is_none()
            && self.setup_failure().is_none()
            && self.current_outcome().is_none()
    }
}

impl fmt::Debug for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OfflineWriteFailure")
            .field("phase", &self.phase)
            .field(
                "cause",
                &PresentedCause {
                    cause: &self.cause,
                    known: self.current_outcome(),
                },
            )
            .field("current_outcome", &self.current_outcome())
            .field("last_completed", &self.last_completed())
            .finish()
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "offline write {:?} failed: {}",
            self.phase,
            PresentedCause {
                cause: &self.cause,
                known: self.current_outcome(),
            }
        )?;
        if let Some(setup) = self.setup_failure() {
            write!(f, ": {setup}")?;
        }
        if let Some(known) = self.current_outcome() {
            write!(f, "; current known result: {known:?}")?;
        }
        if let Some(previous) = self.last_completed() {
            write!(f, "; previous completed result: {previous:?}")?;
        }

        match self.phase {
            Phase::Prepare if matches!(self.cause, Cause::Prepare(_)) => {
                f.write_str("; this prepare did not change target bytes; retain the journal for diagnosis; do not retry the mutation")?;
            }
            Phase::Commit | Phase::Recovery => {
                f.write_str("; retain the target and journal; qualified recovery or diagnosis is required; do not retry the mutation")?;
            }
            Phase::Confirmation => {
                f.write_str("; retain the target and retired bundle; no active-recovery replay or fallback is authorized")?;
            }
            Phase::Finalization if self.current_outcome().is_some() => {
                f.write_str("; known completion is not undone; retain remaining journal resources for diagnosis; do not replay UPDATE")?;
            }
            Phase::Finalization => {
                f.write_str("; finalization is unresolved; retain remaining resources for diagnosis; do not replay UPDATE")?;
            }
            Phase::Setup => {
                f.write_str("; retain the target and selected namespace for diagnosis; do not overwrite or retry the journal")?;
            }
            Phase::Release => {
                f.write_str(
                    "; release is unconfirmed; no automatic unlock retry or normal access follows",
                )?;
            }
            _ if self.pair_state().is_some() || self.setup_failure().is_some() => {
                f.write_str(
                    "; retained journal state blocks normal access; do not retry the mutation",
                )?;
            }
            _ => {}
        }
        Ok(())
    }
}

impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if matches!(self.cause, Cause::Setup)
            && let Some(failure) = self.setup_failure()
        {
            return Some(failure);
        }
        Some(&self.cause)
    }
}

/// IO uses RecoveryOutcome internally for cleanup association. A confirmed
/// current image must not promote that label into a historical native claim.
struct PresentedCause<'a> {
    cause: &'a Cause,
    known: Option<KnownOutcome>,
}

impl fmt::Display for PresentedCause<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.cause, self.known) {
            (Cause::Finalization(error), Some(KnownOutcome::Verified(_))) => {
                write!(
                    formatter,
                    "journal finalization {:?} failed: {}",
                    error.phase, error.cause
                )
            }
            (cause, _) => fmt::Display::fmt(cause, formatter),
        }
    }
}

impl fmt::Debug for PresentedCause<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
