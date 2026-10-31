use super::storage::PairedStorage;
use super::{
    BindingError, OfflineTarget,
    reopen::{self, ExistingBundle, ReopenError},
};
use super::{
    create,
    retirement::{self, FinalizationError, FinalizationPhase, TerminalPermit},
};
use crate::{IoError, journal::RecoveryOutcome};
use std::{error::Error, fmt, path::Path};
use tablecraft_core::limits::{Resource, ResourceBudget};
use thiserror::Error;

const RETIRED_PREFIX: &[u8] = b".tablecraft-retired-";
pub(super) const NONCE_BYTES: usize = 16;
const HEX_PER_BYTE: usize = 2;
const HEX_SHIFT: u8 = 4;
const HEX_MASK: u8 = 0x0f;
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
pub(super) const RETIRED_NAME_BYTES: usize = RETIRED_PREFIX.len() + NONCE_BYTES * HEX_PER_BYTE;

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

/// Opening an existing bundle writes no namespace/target; failure retains its
/// original target lease for explicit release/diagnosis, not path reopening.
pub struct PairSetupFailure {
    cause: PairError,
    target: OfflineTarget,
    incomplete: Vec<create::IncompleteBundle>,
}

impl PairSetupFailure {
    /// Structured cause without paths, source bytes or automatic retries.
    pub fn cause(&self) -> &PairError {
        &self.cause
    }

    /// Whether setup retained a selected namespace and any opened resources.
    /// This is not proof that a directory exists or that an existing entry is owned.
    pub fn incomplete_setup(&self) -> bool {
        !self.incomplete.is_empty()
    }

    /// Recover the retained target. Dropping incomplete setup resources closes
    /// handles only; possible bundle entries remain for explicit diagnosis.
    /// Recover exactly the retained target owner after a read-only open failure.
    pub fn into_target(self) -> OfflineTarget {
        self.target
    }
}

impl fmt::Debug for PairSetupFailure {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output
            .debug_struct("PairSetupFailure")
            .field("cause", &self.cause)
            .field(
                "setup_phase",
                &self.incomplete.first().map(|bundle| bundle.phase),
            )
            .finish_non_exhaustive()
    }
}

impl fmt::Display for PairSetupFailure {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "journal bundle open failed: {}", self.cause)
    }
}

impl Error for PairSetupFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}

fn opened_error(error: ReopenError) -> PairError {
    match error {
        ReopenError::Io(error) => PairError::Io(error),
        ReopenError::Binding(error) => PairError::Binding(error),
        ReopenError::IdentityMismatch => PairError::IdentityMismatch,
        ReopenError::BindingLength => PairError::Binding(BindingError::InvalidLength),
        ReopenError::IntentMismatch => PairError::IntentMismatch,
        ReopenError::IntentLimit => PairError::IntentLimit,
    }
}

pub(super) fn retirement_name(nonce: &[u8; NONCE_BYTES]) -> [u8; RETIRED_NAME_BYTES] {
    let mut name = [0; RETIRED_NAME_BYTES];
    name[..RETIRED_PREFIX.len()].copy_from_slice(RETIRED_PREFIX);
    for (index, byte) in nonce.iter().enumerate() {
        let start = RETIRED_PREFIX.len() + index * HEX_PER_BYTE;
        name[start] = HEX_DIGITS[(byte >> HEX_SHIFT) as usize];
        name[start + 1] = HEX_DIGITS[(byte & HEX_MASK) as usize];
    }
    name
}

/// Retained target and verified existing bundle, using their actual descriptors.
/// Storage access is explicit recovery-required access; it grants no native
/// write profile, normal-read permission or automatic journal cleanup.
pub struct OfflineJournalPair {
    target: OfflineTarget,
    bundle: ExistingBundle,
    state: PairState,
    known: Option<RecoveryOutcome>,
    before_release: Option<PairState>,
    terminal: TerminalPermit,
}

impl OfflineJournalPair {
    /// Reopen only an existing explicitly selected bundle. The target is moved,
    /// never reopened or upgraded from a shared descriptor. No V1/native program
    /// is parsed or executed by this association operation.
    pub fn open_existing(
        target: OfflineTarget,
        bundle: &Path,
        maximum_intent_bytes: u64,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, PairSetupFailure> {
        Self::open_bound(
            target,
            bundle,
            maximum_intent_bytes,
            false,
            budget,
            cancelled,
        )
    }

    /// Open only an intact reserved retired bundle, without granting write or
    /// normal access. All actual objects/intent must match and the final leaf
    /// must equal this binding nonce's exact retirement name. Missing/partial
    /// bundles remain diagnosis; no pending recovery or SQL runs here.
    pub fn open_retired(
        target: OfflineTarget,
        bundle: &Path,
        maximum_intent_bytes: u64,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, PairSetupFailure> {
        Self::open_bound(
            target,
            bundle,
            maximum_intent_bytes,
            true,
            budget,
            cancelled,
        )
    }

    fn open_bound(
        target: OfflineTarget,
        bundle: &Path,
        maximum_intent_bytes: u64,
        retired: bool,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, PairSetupFailure> {
        let _span = tracing::debug_span!("io.journal.open_pair").entered();
        let terminal = match TerminalPermit::reserve(bundle.as_os_str().len(), budget, cancelled) {
            Ok(terminal) => terminal,
            Err(cause) => {
                return Err(PairSetupFailure {
                    cause: cause.into(),
                    target,
                    incomplete: Vec::new(),
                });
            }
        };
        let opened = match reopen::reopen(&target, bundle, maximum_intent_bytes, budget, cancelled)
        {
            Ok(opened) => opened,
            Err(error) => {
                return Err(PairSetupFailure {
                    cause: opened_error(error),
                    target,
                    incomplete: Vec::new(),
                });
            }
        };
        if let Err(error) = budget.charge(Resource::Steps, RETIRED_NAME_BYTES as u64) {
            return Err(PairSetupFailure {
                cause: PairError::Io(error.into()),
                target,
                incomplete: Vec::new(),
            });
        }
        let reserved_name =
            opened.name.as_bytes() == retirement_name(&opened.binding.nonce).as_slice();
        if reserved_name != retired {
            return Err(PairSetupFailure {
                cause: PairError::ReservedName,
                target,
                incomplete: Vec::new(),
            });
        }
        Ok(Self {
            target,
            bundle: opened,
            state: if retired {
                PairState::RetiredVerificationRequired
            } else {
                PairState::RecoveryRequired
            },
            known: None,
            before_release: None,
            terminal,
        })
    }

    /// Create one new private bundle with retained objects and acknowledged
    /// setup barriers. It never mutates target bytes or reuses an existing bundle.
    pub fn create(
        mut target: OfflineTarget,
        bundle: &Path,
        intent: Vec<u8>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, PairSetupFailure> {
        let terminal = match TerminalPermit::reserve(bundle.as_os_str().len(), budget, cancelled) {
            Ok(terminal) => terminal,
            Err(cause) => {
                return Err(PairSetupFailure {
                    cause: cause.into(),
                    target,
                    incomplete: Vec::new(),
                });
            }
        };
        let opened = match create::create(&mut target, bundle, intent, budget, cancelled) {
            Ok(opened) => opened,
            Err(failed) => {
                return Err(PairSetupFailure {
                    cause: failed.cause,
                    target,
                    incomplete: failed.incomplete,
                });
            }
        };
        Ok(Self {
            target,
            bundle: opened,
            state: PairState::Ready,
            known: None,
            before_release: None,
            terminal,
        })
    }

    /// Exact opaque intent bytes already checked against the actual binding.
    pub fn intent_bytes(&self) -> &[u8] {
        &self.bundle.intent_bytes
    }

    /// Borrow both actual locked devices in one lexical scope. Dropping this
    /// borrow does not acknowledge recovery or permit normal access.
    pub fn storage(&mut self) -> Result<(PairedStorage<'_>, PairedStorage<'_>), PairError> {
        if !matches!(self.state, PairState::Ready | PairState::RecoveryRequired)
            || self.known.is_some()
        {
            return Err(PairError::InvalidState);
        }
        self.state = PairState::RecoveryRequired;
        Ok((
            PairedStorage::new(self.target.storage_mut()?),
            PairedStorage::new(&mut self.bundle.journal),
        ))
    }

    /// Lend read/seek/sync-only views for completed-image confirmation. Both
    /// Write and flush are denied before invoking the underlying descriptor.
    pub fn confirmation_storage(
        &mut self,
    ) -> Result<(PairedStorage<'_>, PairedStorage<'_>), PairError> {
        if self.state != PairState::RetiredVerificationRequired || self.known.is_some() {
            return Err(PairError::InvalidState);
        }
        Ok((
            PairedStorage::confirmation(self.target.storage_mut()?),
            PairedStorage::confirmation(&mut self.bundle.journal),
        ))
    }

    /// Record a write-free native confirmation, then acknowledge only the held
    /// parent namespace barrier. This never renames or replays pending recovery.
    pub fn acknowledge_retired(
        &mut self,
        outcome: RecoveryOutcome,
    ) -> Result<(), FinalizationError> {
        let _span = tracing::debug_span!("io.journal.acknowledge_retired").entered();
        if self.state != PairState::RetiredVerificationRequired || self.known.is_some() {
            return Err(FinalizationError {
                phase: FinalizationPhase::State,
                outcome: self.known,
                cause: PairError::InvalidState,
            });
        }
        self.known = Some(outcome);
        retirement::acknowledge_completed(
            &self.bundle,
            &mut self.state,
            &mut self.terminal,
            outcome,
        )
    }

    /// Record an already acknowledged native/generic outcome before any fallible
    /// namespace action. This data is not a substitute for executing that protocol.
    pub fn note_outcome(&mut self, outcome: RecoveryOutcome) -> Result<(), PairError> {
        if self.state != PairState::RecoveryRequired || self.known.is_some() {
            return Err(PairError::InvalidState);
        }
        self.known = Some(outcome);
        self.state = PairState::OutcomeRecorded;
        Ok(())
    }

    /// Record known native completion FIRST, then use only prepaid namespace
    /// work. No shared cancellation/limit callback follows that known result.
    pub fn retire(&mut self, outcome: RecoveryOutcome) -> Result<(), FinalizationError> {
        let _span = tracing::debug_span!("io.journal.retire_pair").entered();
        if self.known.is_none() {
            self.note_outcome(outcome)
                .map_err(|cause| FinalizationError {
                    phase: FinalizationPhase::State,
                    outcome: self.known,
                    cause,
                })?;
        }
        if self.known != Some(outcome) {
            return Err(FinalizationError {
                phase: FinalizationPhase::State,
                outcome: self.known,
                cause: PairError::InvalidState,
            });
        }
        retirement::retire(&self.bundle, &mut self.state, &mut self.terminal, outcome)
    }

    /// Explicitly reattempt ONLY the held barrier after known-successful rename.
    /// Unknown rename outcomes require diagnosis and cannot reach this action.
    pub fn confirm_retirement(
        &mut self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), FinalizationError> {
        let outcome = self.known.ok_or(FinalizationError {
            phase: FinalizationPhase::State,
            outcome: None,
            cause: PairError::InvalidState,
        })?;
        retirement::confirm(&self.bundle, &mut self.state, outcome, budget, cancelled)
    }

    /// Remove only the owned retired bundle, preserving known outcome even if
    /// cleanup takes effect partially. No active journal or target is deleted.
    pub fn cleanup_retired(&mut self) -> Result<(), FinalizationError> {
        let _span = tracing::debug_span!("io.journal.cleanup_retired_pair").entered();
        let outcome = self.known.ok_or(FinalizationError {
            phase: FinalizationPhase::State,
            outcome: None,
            cause: PairError::InvalidState,
        })?;
        retirement::cleanup(&self.bundle, &mut self.state, &mut self.terminal, outcome)
    }

    /// Namespace phase survives an uncertain or successful lock release.
    pub fn namespace_state(&self) -> PairState {
        self.before_release.unwrap_or(self.state)
    }

    /// Current host pairing phase, separate from actual native qualification.
    pub const fn state(&self) -> PairState {
        self.state
    }

    /// Known completion survives any subsequent cleanup/release failure.
    pub const fn known_outcome(&self) -> Option<RecoveryOutcome> {
        self.known
    }

    /// Explicitly release locks without deleting any bundle/target entry. Release
    /// does not mean retirement or normal-access readiness. Any failure retains
    /// owners but invalidates the claim that both leases are still held.
    pub fn release(&mut self) -> Result<(), PairError> {
        let _span = tracing::debug_span!("io.journal.release_pair").entered();
        if matches!(
            self.state,
            PairState::ReleaseUncertain | PairState::Released
        ) {
            return Err(PairError::InvalidState);
        }
        self.before_release = Some(self.state);
        self.state = PairState::ReleaseUncertain;
        self.bundle.journal.try_release()?;
        self.target.release()?;
        self.state = PairState::Released;
        Ok(())
    }
}
