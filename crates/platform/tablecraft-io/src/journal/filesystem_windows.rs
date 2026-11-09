//! Local-NTFS flat journal pairing with explicit retained artifact outcomes.
#[path = "filesystem_windows/pair.rs"]
mod pair;
#[path = "filesystem_windows/setup.rs"]
mod setup;
#[path = "filesystem_windows/storage.rs"]
mod storage;
#[path = "filesystem_windows/target.rs"]
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
#[path = "filesystem_windows/tests.rs"]
mod tests;
