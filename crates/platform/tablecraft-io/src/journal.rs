//! Bounded versioned image transactions over explicitly paired storage.
//!
//! Hosts must hold exclusive access to distinct target/journal resources, bind
//! the correct pair, and make journal creation/directory entries durable before
//! prepare. File sync alone cannot do that. This module never creates, deletes,
//! discovers or reuses journals, and does not implement a native database format.
//! Every prepared/failed commit must be recovered before normal target access.
//! A completed journal remains intact for host-controlled durable cleanup.
mod error;
pub use error::{CommitError, JournalError};
mod region;
pub use region::BoundedJournalRegion;
mod storage;
pub use storage::{DurableStorage, ResizableDurableStorage};
mod resizable;
pub use resizable::{
    CheckedResizableImage, CheckedV2, CheckedV3, PreparedResizableImage, ResizableState, V2State,
    V3State, inspect_v2, inspect_v3, prepare_resizable_expected, prepare_resizable_v3_expected,
    recover_resizable, recover_resizable_v3,
};
mod format;
mod recovery;
pub use recovery::{RecoveryOutcome, recover};
mod checked_v1;
pub use checked_v1::{CheckedV1, V1State, inspect_v1};
mod transaction;
pub use transaction::{PreparedImage, prepare, prepare_expected};
#[cfg(test)]
mod tests;

#[cfg(feature = "filesystem-journal")]
pub mod filesystem_contracts;

/// Explicit retained Unix filesystem pairing; native permission stays with its consumer.
#[cfg(all(unix, feature = "filesystem-journal"))]
pub mod filesystem;

#[cfg(all(windows, feature = "filesystem-journal"))]
#[path = "journal/filesystem_windows.rs"]
pub mod filesystem;

#[cfg(all(feature = "filesystem-journal", any(windows, test)))]
mod flat_binding;

#[cfg(all(feature = "filesystem-journal", any(windows, test)))]
mod flat_retirement;

mod checked_image;
pub use checked_image::{CheckedImage, inspect_image};
