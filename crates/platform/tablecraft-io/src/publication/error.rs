use crate::IoError;
use thiserror::Error;

/// Publication failures distinguish absence of publication from visible uncertainty.
/// No variant authorizes retrying publication or deleting the final entry.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PublicationError {
    /// This platform or filesystem has no implemented publication guarantee.
    #[error("required publication durability is unsupported on this platform or filesystem")]
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
    /// The final entry is visible but its platform durability step failed.
    #[error("output is published but namespace durability is unconfirmed: {cause}")]
    PublishedUnconfirmed {
        /// Failure of the platform's terminal durability operation.
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
    #[cfg(any(unix, windows, test))]
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
    #[cfg(all(not(windows), any(unix, test)))]
    pub(super) device: u64,
    #[cfg(all(not(windows), any(unix, test)))]
    pub(super) inode: u64,
    #[cfg(windows)]
    pub(super) volume: u64,
    #[cfg(windows)]
    pub(super) file_id: [u8; 16],
}

/// The acknowledged platform operation; these guarantees are distinct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicationGuarantee {
    /// The captured Unix parent directory acknowledged its terminal barrier.
    CapturedParentBarrier,
    /// A qualified local NTFS file was renamed through its write-through handle,
    /// then that retained file acknowledged its terminal flush. This does not
    /// attest a separately flushed parent or ancestor directory.
    NtfsWriteThroughRename,
}

/// Acknowledged publication under its explicit platform guarantee.
/// Durability remains subject to the filesystem/device honoring its barriers.
pub struct PublicationReceipt {
    pub(super) parent: ParentIdentity,
    pub(super) guarantee: PublicationGuarantee,
}

impl PublicationReceipt {
    /// The specific durability operation acknowledged by this publication.
    pub const fn guarantee(&self) -> PublicationGuarantee {
        self.guarantee
    }

    /// Whether the captured parent acknowledged a separate namespace barrier.
    /// NTFS write-through rename publication does not make this claim.
    pub const fn namespace_synced(&self) -> bool {
        matches!(self.guarantee, PublicationGuarantee::CapturedParentBarrier)
    }

    /// The operation's captured parent, independent of later pathname retargeting.
    pub fn parent_identity(&self) -> &ParentIdentity {
        &self.parent
    }
}
