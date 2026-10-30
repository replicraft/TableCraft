use crate::IoError;
use thiserror::Error;

/// Publication failures distinguish absence of publication from visible uncertainty.
/// No variant authorizes retrying publication or deleting the final entry.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PublicationError {
    /// This platform has no implemented required namespace barrier.
    #[error("required namespace durability is unsupported on this platform")]
    UnsupportedDurability,
    /// No final publication succeeded; cleanup itself may have failed.
    #[error("output was not published: {cause}; temporary may remain: {temporary_may_remain}")]
    NotPublished {
        /// Primary failure, without paths or host messages.
        #[source]
        cause: IoError,
        /// A subsequent owned-temporary cleanup failure.
        cleanup: Option<IoError>,
        /// An owned temporary may remain; this does not assert its presence.
        temporary_may_remain: bool,
    },
    /// The namespace callback failed without an established visibility outcome.
    #[error("publication visibility is uncertain: {cause}; do not retry automatically")]
    VisibilityUncertain {
        /// Namespace callback failure.
        #[source]
        cause: IoError,
    },
    /// The final entry is visible but its parent barrier was not acknowledged.
    #[error("output is published but namespace durability is unconfirmed: {cause}")]
    PublishedUnconfirmed {
        /// Parent barrier failure.
        #[source]
        cause: IoError,
    },
    /// Publication succeeded, but an owned temporary may remain.
    #[error(
        "output is published; temporary cleanup failed: {cause}; namespace synced: {namespace_synced}"
    )]
    PublishedCleanupFailure {
        /// Cleanup failure, without paths or host messages.
        #[source]
        cause: IoError,
        /// Whether the captured parent acknowledged its terminal barrier.
        namespace_synced: bool,
        /// Independent terminal barrier failure, if any.
        barrier: Option<IoError>,
    },
}

impl PublicationError {
    #[cfg(any(unix, test))]
    pub(super) fn before(cause: IoError) -> Self {
        Self::NotPublished {
            cause,
            cleanup: None,
            temporary_may_remain: false,
        }
    }
}

/// Opaque identity of the directory captured for this operation.
/// It is not a persistent identity or a guarantee about a later pathname lookup.
#[derive(Clone, PartialEq, Eq)]
pub struct ParentIdentity {
    #[cfg(any(unix, test))]
    pub(super) device: u64,
    #[cfg(any(unix, test))]
    pub(super) inode: u64,
}

/// Acknowledged publication and namespace barrier in the captured directory.
/// Durability remains subject to the filesystem/device honoring its barriers.
pub struct PublicationReceipt {
    pub(super) parent: ParentIdentity,
}

impl PublicationReceipt {
    /// The captured parent acknowledged the terminal namespace barrier.
    pub const fn namespace_synced(&self) -> bool {
        true
    }

    /// The operation's captured parent, independent of later pathname retargeting.
    pub fn parent_identity(&self) -> &ParentIdentity {
        &self.parent
    }
}
