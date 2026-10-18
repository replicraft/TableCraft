//! Bounded fixed-length image transactions over explicitly paired storage.
//!
//! Hosts must hold exclusive access to distinct target/journal resources, bind
//! the correct pair, and make journal creation/directory entries durable before
//! prepare. File sync alone cannot do that. This module never creates, deletes,
//! discovers or reuses journals, and does not implement a native database format.
//! Every prepared/failed commit must be recovered before normal target access.
//! A completed journal remains intact for host-controlled durable cleanup.
mod error;
pub use error::{CommitError, JournalError};
mod storage;
pub use storage::DurableStorage;
mod format;
mod recovery;
pub use recovery::{RecoveryOutcome, recover};
mod transaction;
pub use transaction::{PreparedImage, prepare, prepare_expected};
#[cfg(test)]
mod tests;
