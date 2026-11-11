use crate::SnapshotError;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_SNAPSHOT: AtomicU64 = AtomicU64::new(1);

pub(super) fn issue() -> Result<u64, SnapshotError> {
    NEXT_SNAPSHOT
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
            current.checked_add(1)
        })
        .map_err(|_| SnapshotError::IdentityExhausted)
}

/// Opaque identity of one validated complete snapshot, including an empty one.
/// Reconstructing identical metadata and rows issues a different identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapshotId(pub(super) u64);

/// Opaque process-local snapshot position, not a persistent backend RecordId.
/// Only its issuing snapshot can resolve it. Reopening/rebuilding invalidates it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowHandle {
    pub(super) snapshot: u64,
    pub(super) ordinal: usize,
}
