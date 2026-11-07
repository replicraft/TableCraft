//! Local-NTFS flat journal pairing with explicit retained artifact outcomes.
mod pair;
mod setup;
mod storage;
mod target;

pub use super::filesystem_contracts::{
    BindingError, CleanupDisposition, FinalizationError, FinalizationPhase, PairError, PairState,
    RetiredCleanupPolicy, RetirementGuarantee, TargetLockState,
};
pub use pair::{OfflineJournalPair, PairSetupFailure, TargetTransferFailure};
pub use storage::{PairedStorage, ResizablePairedTarget};
pub use target::OfflineTarget;

use super::flat_binding::FlatIdentity;
use crate::windows_fs::FileIdentity;

fn binding_identity(identity: FileIdentity) -> FlatIdentity {
    FlatIdentity {
        volume: identity.volume,
        file_id: identity.file_id,
    }
}

#[cfg(test)]
mod tests;
