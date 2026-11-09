//! Explicit current-database settings; presentation never grants native authority.
mod appearance;
mod confirmation;
mod options;
mod rendering;
mod state;
#[cfg(test)]
mod tests;

use super::{EditSaveOutcome, SaveCleanupNotice};

/// Exact inert observation, preserving absence separately from explicit zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameAutoCorrectState {
    Missing,
    Absent,
    ExplicitOff,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DatabaseOptionsSaveIdentity {
    pub document_revision: u64,
    pub request_id: u64,
    pub review_id: u64,
    pub save_id: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DatabaseOptionsSaveCommand {
    Cancel,
    Recover,
    Release,
    Acknowledge,
    RetryReopen,
    ReturnToSettings,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Phase {
    #[default]
    Idle,
    Confirming,
    Inspecting {
        request_id: u64,
    },
    Reviewing {
        request_id: u64,
        cancelled: bool,
    },
    Reviewed {
        request_id: u64,
        review_id: u64,
    },
    Saving {
        identity: DatabaseOptionsSaveIdentity,
        cancelled: bool,
        resolving: bool,
    },
    Finished {
        identity: DatabaseOptionsSaveIdentity,
    },
    Reopening {
        identity: DatabaseOptionsSaveIdentity,
        cancelled: bool,
    },
    ReopenFailed {
        identity: DatabaseOptionsSaveIdentity,
    },
    Warning {
        identity: DatabaseOptionsSaveIdentity,
        open: bool,
    },
}

#[derive(Default)]
pub(in crate::workspace) struct Model {
    revision: Option<u64>,
    observed: Option<(NameAutoCorrectState, bool)>,
    phase: Phase,
    outcome: Option<EditSaveOutcome>,
    error: Option<String>,
    omitted: bool,
    cleanup: Option<SaveCleanupNotice>,
    pub(in crate::workspace) confirmation: Option<CloseGraph>,
}

impl Model {
    pub(in crate::workspace) fn heap_bytes(&self) -> Option<usize> {
        self.error
            .as_ref()
            .map_or(0, String::capacity)
            .checked_add(self.outcome.as_ref().map_or(0, outcome_bytes))?
            .checked_add(
                self.confirmation
                    .as_ref()
                    .map_or(Some(0), CloseGraph::heap_bytes)?,
            )
    }

    fn identity(&self) -> Option<DatabaseOptionsSaveIdentity> {
        match self.phase {
            Phase::Saving { identity, .. }
            | Phase::Finished { identity }
            | Phase::Reopening { identity, .. }
            | Phase::ReopenFailed { identity }
            | Phase::Warning { identity, .. } => Some(identity),
            _ => None,
        }
    }
}

fn outcome_bytes(outcome: &EditSaveOutcome) -> usize {
    match outcome {
        EditSaveOutcome::Saved(text)
        | EditSaveOutcome::Cancelled(text)
        | EditSaveOutcome::Failed(text)
        | EditSaveOutcome::Retained { message: text, .. } => text.capacity(),
    }
}

/// Presentation is reserved before actor start; binding its real ID cannot fail.
pub struct PreparedDatabaseOptionsSave<'a> {
    model: &'a mut Model,
    document_revision: u64,
    request_id: u64,
    review_id: u64,
    bound: bool,
    restore: Phase,
    resolving: bool,
}

impl PreparedDatabaseOptionsSave<'_> {
    pub fn bind(mut self, save_id: u64) -> DatabaseOptionsSaveIdentity {
        let identity = DatabaseOptionsSaveIdentity {
            document_revision: self.document_revision,
            request_id: self.request_id,
            review_id: self.review_id,
            save_id,
        };
        self.model.phase = Phase::Saving {
            identity,
            cancelled: false,
            resolving: self.resolving,
        };
        if self.resolving {
            self.model.outcome = None;
        }
        self.bound = true;
        identity
    }
}

impl Drop for PreparedDatabaseOptionsSave<'_> {
    fn drop(&mut self) {
        if !self.bound {
            self.model.phase = self.restore;
        }
    }
}

/// UI close approval only; it contains no native review or persistence authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DatabaseOptionsCloseRequest {
    pub document_revision: u64,
    pub confirmation_id: u64,
}

pub(in crate::workspace) struct CloseGraph {
    pub(in crate::workspace) request: DatabaseOptionsCloseRequest,
    pub(in crate::workspace) tabs: Vec<super::ObjectTab>,
    pub(in crate::workspace) active: Option<u64>,
    pub(in crate::workspace) approved: bool,
}

impl CloseGraph {
    pub(in crate::workspace) fn heap_bytes(&self) -> Option<usize> {
        let mut bytes = self
            .tabs
            .capacity()
            .checked_mul(size_of::<super::ObjectTab>())?;
        for tab in &self.tabs {
            let extra = match &tab.target.kind {
                super::ObjectKind::Table { name } | super::ObjectKind::SavedQuery { name, .. } => {
                    name.capacity()
                }
                super::ObjectKind::Sql => 0,
                super::ObjectKind::DraftTable { .. } => return None,
            };
            bytes = bytes.checked_add(extra)?;
        }
        Some(bytes)
    }
}
