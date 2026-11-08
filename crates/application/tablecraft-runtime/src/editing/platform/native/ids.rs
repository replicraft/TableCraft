//! One process-wide identity space for both reviewed editing domains.
use crate::Error;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

pub(super) fn issue_id() -> Result<u64, Error> {
    NEXT_ID
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .map(|previous| previous + 1)
        .map_err(|_| Error::RevisionExhausted)
}
