//! Consume a clean pair without releasing or reopening its original target.
use super::{OfflineJournalPair, OfflineTarget, PairError, PairState, TargetLockState};
use crate::{ExclusiveFile, IoError};
use std::{error::Error, fmt};

/// A refused transfer retaining the exact pair and its separately known outcome.
/// No target is returned on failure, even if journal unlock may have taken effect.
#[must_use = "retain the pair for diagnosis; failure does not imply rollback"]
pub struct TargetTransferFailure {
    cause: PairError,
    pair: OfflineJournalPair,
}

impl TargetTransferFailure {
    /// Sanitized refusal or journal-unlock cause, without paths or payloads.
    pub fn cause(&self) -> &PairError {
        &self.cause
    }

    /// Retained lifecycle and known-outcome facts; no mutable storage is lent.
    pub fn pair(&self) -> &OfflineJournalPair {
        &self.pair
    }

    /// Recover both the typed cause and the same retained pair without allocation.
    pub fn into_parts(self) -> (PairError, OfflineJournalPair) {
        (self.cause, self.pair)
    }
}

impl fmt::Debug for TargetTransferFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TargetTransferFailure")
            .field("cause", &self.cause)
            .field("state", &self.pair.state())
            .field("known_outcome", &self.pair.known_outcome())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for TargetTransferFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "journal target transfer failed: {}", self.cause)
    }
}

impl Error for TargetTransferFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}

impl OfflineJournalPair {
    /// Return the same still-exclusively-held target only after acknowledged
    /// retirement and garbage cleanup, with a known completion outcome.
    /// Journal unlock must acknowledge before this owner is returned. The target
    /// is never unlocked, cloned or reopened, and no terminal allocation occurs.
    ///
    /// Any other lifecycle state is retained unchanged. Journal unlock failure
    /// retains a release-uncertain pair and its known outcome for diagnosis; it
    /// cannot be retried through this transfer. Existing `release` is unchanged.
    #[allow(clippy::result_large_err)] // Retain the pair inline; terminal boxing would allocate.
    pub fn into_target_after_clean(self) -> Result<OfflineTarget, TargetTransferFailure> {
        let _span = tracing::debug_span!("io.journal.transfer_clean_target").entered();
        self.transfer_with(ExclusiveFile::try_release)
    }

    /// Return the same held target after acknowledged intact retirement without
    /// deleting the retired artifact. A partially cleaned or already clean pair
    /// is refused; callers must retain the artifact for explicit later disposal.
    /// Journal unlock failure retains both owners with uncertain release state.
    #[allow(clippy::result_large_err)] // Retain the pair without terminal allocation.
    pub fn into_target_after_retirement(self) -> Result<OfflineTarget, TargetTransferFailure> {
        let _span = tracing::debug_span!("io.journal.transfer_retired_target").entered();
        self.transfer_retired_with(ExclusiveFile::try_release)
    }

    #[allow(clippy::result_large_err)]
    fn transfer_retired_with(
        self,
        release_journal: impl FnOnce(&mut ExclusiveFile) -> Result<(), IoError>,
    ) -> Result<OfflineTarget, TargetTransferFailure> {
        self.transfer_in_state(PairState::Retired, release_journal)
    }

    #[allow(clippy::result_large_err)] // Same nonallocating ownership contract as the public entry.
    fn transfer_with(
        self,
        release_journal: impl FnOnce(&mut ExclusiveFile) -> Result<(), IoError>,
    ) -> Result<OfflineTarget, TargetTransferFailure> {
        self.transfer_in_state(PairState::RetiredClean, release_journal)
    }

    #[allow(clippy::result_large_err)]
    fn transfer_in_state(
        mut self,
        required: PairState,
        release_journal: impl FnOnce(&mut ExclusiveFile) -> Result<(), IoError>,
    ) -> Result<OfflineTarget, TargetTransferFailure> {
        if self.state != required
            || self.known.is_none()
            || self.target.lock_state() != TargetLockState::Held
        {
            return Err(TargetTransferFailure {
                cause: PairError::InvalidState,
                pair: self,
            });
        }
        self.before_release = Some(self.state);
        self.state = PairState::ReleaseUncertain;
        if let Err(error) = release_journal(&mut self.bundle.journal) {
            return Err(TargetTransferFailure {
                cause: PairError::Io(error),
                pair: self,
            });
        }
        // Remaining retired bundle handles close through their existing owners.
        // Moving the target does not invoke its release or destructor.
        let OfflineJournalPair { target, .. } = self;
        Ok(target)
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
#[path = "transfer/tests.rs"]
mod tests;
