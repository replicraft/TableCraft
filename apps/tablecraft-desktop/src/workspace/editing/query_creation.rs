//! Retain one exact authoring owner through native creation and guarded reopen.
mod reopen;
mod review;
mod save;

use crate::workspace::filtering::RestoreFilter;
use std::sync::Arc;
use tablecraft_runtime::{QueryCreatedReopen, SaveOutcome};
use tablecraft_ui::{QueryCreationRequest, QueryCreationSaveIdentity, SaveCleanupNotice};

/// The native worker and its terminal owner survive unrelated presentation changes.
/// This owner retains the exact shared authoring snapshot, never a replacement SQL draft.
pub(in crate::workspace) struct QueryCreationHost {
    request_id: u64,
    request: Arc<QueryCreationRequest>,
    review_id: Option<u64>,
    witness: Option<Arc<QueryCreatedReopen>>,
    status: Option<QueryCreationSaveIdentity>,
    terminal: Option<SaveOutcome>,
    cleanup: Option<SaveCleanupNotice>,
    filters: Vec<RestoreFilter>,
    reopening: bool,
    cancellation_requested: bool,
}

impl QueryCreationHost {
    fn witnessed_reopen_authorized(&self) -> bool {
        self.terminal.is_some_and(|outcome| {
            outcome
                .query_creation_reopen_facts()
                .is_ok_and(|facts| facts.replacement)
        })
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
