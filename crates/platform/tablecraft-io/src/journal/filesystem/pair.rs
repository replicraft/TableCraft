use super::storage::PairedStorage;
use super::{
    BindingError, OfflineTarget,
    reopen::{self, ExistingBundle, ReopenError},
};
use crate::{IoError, journal::RecoveryOutcome};
use std::{error::Error, fmt, path::Path};
use tablecraft_core::limits::{Resource, ResourceBudget};
use thiserror::Error;

const RETIRED_PREFIX: &[u8] = b".tablecraft-retired-";
const NONCE_BYTES: usize = 16;
const HEX_PER_BYTE: usize = 2;
const HEX_SHIFT: u8 = 4;
const HEX_MASK: u8 = 0x0f;
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
const RETIRED_NAME_BYTES: usize = RETIRED_PREFIX.len() + NONCE_BYTES * HEX_PER_BYTE;

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
}

impl PairSetupFailure {
    /// Structured cause without paths, source bytes or automatic retries.
    pub fn cause(&self) -> &PairError {
        &self.cause
    }

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

fn retirement_name(nonce: &[u8; NONCE_BYTES]) -> [u8; RETIRED_NAME_BYTES] {
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
        let _span = tracing::debug_span!("io.journal.open_pair").entered();
        let opened = match reopen::reopen(&target, bundle, maximum_intent_bytes, budget, cancelled)
        {
            Ok(opened) => opened,
            Err(error) => {
                return Err(PairSetupFailure {
                    cause: opened_error(error),
                    target,
                });
            }
        };
        if let Err(error) = budget.charge(Resource::Steps, RETIRED_NAME_BYTES as u64) {
            return Err(PairSetupFailure {
                cause: PairError::Io(error.into()),
                target,
            });
        }
        if opened.name.as_bytes() == retirement_name(&opened.binding.nonce).as_slice() {
            return Err(PairSetupFailure {
                cause: PairError::ReservedName,
                target,
            });
        }
        Ok(Self {
            target,
            bundle: opened,
            state: PairState::RecoveryRequired,
            known: None,
        })
    }

    /// Exact opaque intent bytes already checked against the actual binding.
    pub fn intent_bytes(&self) -> &[u8] {
        &self.bundle.intent_bytes
    }

    /// Borrow both actual locked devices in one lexical scope. Dropping this
    /// borrow does not acknowledge recovery or permit normal access.
    pub fn storage(&mut self) -> Result<(PairedStorage<'_>, PairedStorage<'_>), PairError> {
        if self.state != PairState::RecoveryRequired || self.known.is_some() {
            return Err(PairError::InvalidState);
        }
        self.state = PairState::RecoveryRequired;
        Ok((
            PairedStorage::new(self.target.storage_mut()?),
            PairedStorage::new(&mut self.bundle.journal),
        ))
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
        if matches!(
            self.state,
            PairState::ReleaseUncertain | PairState::Released
        ) {
            return Err(PairError::InvalidState);
        }
        self.state = PairState::ReleaseUncertain;
        self.bundle.journal.try_release()?;
        self.target.release()?;
        self.state = PairState::Released;
        Ok(())
    }
}
