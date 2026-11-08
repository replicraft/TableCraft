//! Query-specific presentation ownership; no database or runtime authority.
mod dialog;

use super::{Action, EditSaveOutcome, SavedQuerySqlRequest, State};
use crate::workspace::{objects::memory::Counter, query::parameters::copy_text};

/// Distinguishes a real native save from a read-only unchanged-definition review.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SavedQuerySqlSaveOperation {
    PreparedReview,
    NativeSave { save_id: u64 },
    UnchangedReview,
}

/// Exact host request/review/operation association, without executable authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedQuerySqlSaveIdentity {
    pub request_id: u64,
    pub review_id: u64,
    pub operation: SavedQuerySqlSaveOperation,
}

/// Explicit user intent; only the owning host can perform these operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SavedQuerySqlSaveCommand {
    Cancel,
    Recover,
    Release,
    Acknowledge,
    RetryReopen,
    ReturnToDraft,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Preparing,
    Saving { cancelled: bool, resolving: bool },
    Finished,
    Reopening { cancelled: bool },
    ReopenFailed,
}

pub(super) struct SaveStatus {
    identity: SavedQuerySqlSaveIdentity,
    request: SavedQuerySqlRequest,
    phase: Phase,
    outcome: Option<EditSaveOutcome>,
    reopen_error: Option<String>,
    report_omitted: bool,
    reopen_error_omitted: bool,
}

impl SaveStatus {
    pub(in crate::workspace) fn retained_bytes(&self) -> Option<usize> {
        let mut count = Counter::new(size_of::<Self>());
        count.string(&self.request.target.name);
        count.optional(&self.request.original_sql);
        count.string(&self.request.proposed_sql);
        count.add(self.outcome.as_ref().map_or(0, outcome_bytes));
        count.optional(&self.reopen_error);
        count.finish()
    }
}

impl State {
    /// Admit the full owned status before starting a native actor. No native
    /// save ID or persisted outcome is invented during this preparation.
    pub fn prepare_saved_query_sql_save(
        &mut self,
        request_id: u64,
        request: &SavedQuerySqlRequest,
        review_id: u64,
    ) -> Option<SavedQuerySqlSaveIdentity> {
        self.begin_saved_sql_status(
            request_id,
            request,
            review_id,
            SavedQuerySqlSaveOperation::PreparedReview,
            Phase::Preparing,
        )
    }

    /// Bind the actual accepted actor ID without allocation. Hosts prepare,
    /// start and bind synchronously on their UI thread, without yielding.
    pub fn bind_saved_query_sql_save(
        &mut self,
        prepared: SavedQuerySqlSaveIdentity,
        save_id: u64,
    ) -> Option<SavedQuerySqlSaveIdentity> {
        let status = self.saved_query_sql_save.as_mut()?;
        if status.identity != prepared || status.phase != Phase::Preparing {
            return None;
        }
        status.identity.operation = SavedQuerySqlSaveOperation::NativeSave { save_id };
        status.phase = Phase::Saving {
            cancelled: false,
            resolving: false,
        };
        Some(status.identity)
    }

    /// A refused actor start can discard only its unbound prepared presentation.
    pub fn abort_saved_query_sql_save(&mut self, prepared: SavedQuerySqlSaveIdentity) -> bool {
        let matches = self
            .saved_query_sql_save
            .as_ref()
            .is_some_and(|status| status.identity == prepared && status.phase == Phase::Preparing);
        if matches {
            self.saved_query_sql_save = None;
        }
        matches
    }

    /// An unchanged review performs only a host-owned full-witness read reopen.
    /// No save ID, commit receipt or persisted-success outcome is fabricated.
    pub fn begin_saved_query_sql_unchanged_reopen(
        &mut self,
        request_id: u64,
        request: &SavedQuerySqlRequest,
        review_id: u64,
    ) -> Option<SavedQuerySqlSaveIdentity> {
        self.begin_saved_sql_status(
            request_id,
            request,
            review_id,
            SavedQuerySqlSaveOperation::UnchangedReview,
            Phase::Reopening { cancelled: false },
        )
    }

    fn begin_saved_sql_status(
        &mut self,
        request_id: u64,
        request: &SavedQuerySqlRequest,
        review_id: u64,
        operation: SavedQuerySqlSaveOperation,
        phase: Phase,
    ) -> Option<SavedQuerySqlSaveIdentity> {
        if self.saved_query_sql_save.is_some()
            || !self.matches_saved_query_sql_review(request_id, request, review_id)
        {
            return None;
        }
        let mut estimate = Counter::new(size_of::<SaveStatus>());
        estimate.text(&request.target.name);
        if let Some(original) = &request.original_sql {
            estimate.text(original);
        }
        estimate.text(&request.proposed_sql);
        if !self.admit_object_bytes(estimate.finish()) {
            return None;
        }
        let Some(request) = copy_request(request) else {
            return None;
        };
        let status = SaveStatus {
            identity: SavedQuerySqlSaveIdentity {
                request_id,
                review_id,
                operation,
            },
            request,
            phase,
            outcome: None,
            reopen_error: None,
            report_omitted: false,
            reopen_error_omitted: false,
        };
        if !self.admit_object_bytes(status.retained_bytes()) {
            return None;
        }
        let identity = status.identity;
        self.saved_query_sql_save = Some(status);
        Some(identity)
    }

    pub fn saved_query_sql_save_open(&self) -> bool {
        self.saved_query_sql_save.is_some()
    }

    pub fn saved_query_sql_save_status(
        &self,
    ) -> Option<(SavedQuerySqlSaveIdentity, Option<&EditSaveOutcome>)> {
        let status = self.saved_query_sql_save.as_ref()?;
        Some((status.identity, status.outcome.as_ref()))
    }

    pub fn saved_query_sql_save_request(
        &self,
        identity: SavedQuerySqlSaveIdentity,
    ) -> Option<&SavedQuerySqlRequest> {
        self.saved_query_sql_save
            .as_ref()
            .filter(|status| status.identity == identity)
            .map(|status| &status.request)
    }

    pub fn matches_saved_query_sql_save(&self, identity: SavedQuerySqlSaveIdentity) -> bool {
        self.saved_query_sql_save
            .as_ref()
            .is_some_and(|status| status.identity == identity)
    }

    /// A parked query or fresh catalog publication does not discard its outcome.
    pub fn set_saved_query_sql_save_outcome(
        &mut self,
        identity: SavedQuerySqlSaveIdentity,
        outcome: EditSaveOutcome,
    ) -> bool {
        if !self.saved_query_sql_save.as_ref().is_some_and(|status| {
            status.identity == identity
                && matches!(
                    identity.operation,
                    SavedQuerySqlSaveOperation::NativeSave { .. }
                )
                && matches!(status.phase, Phase::Saving { .. } | Phase::Finished)
        }) {
            return false;
        }
        // Once the actor owns native resources, the terminal kind and recovery
        // affordances must remain publishable even at the UI memory ceiling.
        let omitted = !self.admit_object_bytes(Some(outcome_bytes(&outcome)));
        let outcome = if omitted {
            without_report(outcome)
        } else {
            outcome
        };
        let status = self.saved_query_sql_save.as_mut().unwrap();
        status.phase = Phase::Finished;
        status.outcome = Some(outcome);
        status.report_omitted = omitted;
        status.reopen_error = None;
        status.reopen_error_omitted = false;
        true
    }

    pub fn can_resolve_saved_query_sql_save(
        &self,
        identity: SavedQuerySqlSaveIdentity,
        recover: bool,
    ) -> bool {
        self.saved_query_sql_save.as_ref().is_some_and(|status| {
            status.identity == identity && status.phase == Phase::Finished
                && matches!(&status.outcome, Some(EditSaveOutcome::Retained { can_recover, can_release, .. })
                    if if recover { *can_recover } else { *can_release })
        })
    }

    /// Call only after the retained actor accepts the new operation ID.
    pub fn begin_saved_query_sql_save_resolution(
        &mut self,
        identity: SavedQuerySqlSaveIdentity,
        next_id: u64,
    ) -> Option<SavedQuerySqlSaveIdentity> {
        if !(self.can_resolve_saved_query_sql_save(identity, true)
            || self.can_resolve_saved_query_sql_save(identity, false))
            || matches!(identity.operation, SavedQuerySqlSaveOperation::NativeSave { save_id } if save_id == next_id)
        {
            return None;
        }
        let status = self.saved_query_sql_save.as_mut().unwrap();
        status.identity.operation = SavedQuerySqlSaveOperation::NativeSave { save_id: next_id };
        status.phase = Phase::Saving {
            cancelled: false,
            resolving: true,
        };
        Some(status.identity)
    }

    pub fn can_acknowledge_saved_query_sql_save(
        &self,
        identity: SavedQuerySqlSaveIdentity,
    ) -> bool {
        self.saved_query_sql_save.as_ref().is_some_and(|status| {
            status.identity == identity
                && status.phase == Phase::Finished
                && matches!(
                    &status.outcome,
                    Some(
                        EditSaveOutcome::Saved(_)
                            | EditSaveOutcome::Cancelled(_)
                            | EditSaveOutcome::Failed(_)
                    )
                )
        })
    }

    /// Host-only presentation transition after acknowledged known replacement;
    /// for a retry, the host must retain the same checked read-only witness.
    pub fn begin_saved_query_sql_reopen(&mut self, identity: SavedQuerySqlSaveIdentity) -> bool {
        let retry = self.saved_query_sql_save.as_ref().is_some_and(|status| {
            status.identity == identity && status.phase == Phase::ReopenFailed
        });
        if !retry && !self.can_acknowledge_saved_query_sql_save(identity) {
            return false;
        }
        let status = self.saved_query_sql_save.as_mut().unwrap();
        status.phase = Phase::Reopening { cancelled: false };
        status.reopen_error = None;
        true
    }

    pub fn can_return_to_saved_query_sql_draft(&self, identity: SavedQuerySqlSaveIdentity) -> bool {
        self.saved_query_sql_save.as_ref().is_some_and(|status| {
            status.identity == identity && status.phase == Phase::ReopenFailed
        })
    }

    pub fn saved_query_sql_reopen_error(
        &self,
        identity: SavedQuerySqlSaveIdentity,
    ) -> Option<&str> {
        self.saved_query_sql_save
            .as_ref()
            .filter(|status| status.identity == identity && status.phase == Phase::ReopenFailed)
            .and_then(|status| status.reopen_error.as_deref())
    }

    pub fn set_saved_query_sql_reopen_failed(
        &mut self,
        identity: SavedQuerySqlSaveIdentity,
        message: String,
    ) -> bool {
        if !self.saved_query_sql_save.as_ref().is_some_and(|status| {
            status.identity == identity && matches!(status.phase, Phase::Reopening { .. })
        }) {
            return false;
        }
        let omitted = !self.admit_object_bytes(Some(message.capacity()));
        let status = self.saved_query_sql_save.as_mut().unwrap();
        status.phase = Phase::ReopenFailed;
        status.reopen_error = Some(if omitted { String::new() } else { message });
        status.reopen_error_omitted = omitted;
        true
    }

    /// The host owns release/reopen preconditions. A button click never clears
    /// status, and an active save or retained native owner cannot be cleared here.
    pub fn clear_saved_query_sql_save(&mut self, identity: SavedQuerySqlSaveIdentity) -> bool {
        let clearable = self.can_acknowledge_saved_query_sql_save(identity)
            || self.saved_query_sql_save.as_ref().is_some_and(|status| {
                status.identity == identity
                    && matches!(status.phase, Phase::Reopening { .. } | Phase::ReopenFailed)
            });
        if clearable {
            self.saved_query_sql_save = None;
        }
        clearable
    }

    pub fn request_saved_query_sql_save_command(
        &mut self,
        identity: SavedQuerySqlSaveIdentity,
        command: SavedQuerySqlSaveCommand,
    ) -> Option<Action> {
        let status = self
            .saved_query_sql_save
            .as_ref()
            .filter(|status| status.identity == identity)?;
        let permitted = match command {
            SavedQuerySqlSaveCommand::Cancel => matches!(
                status.phase,
                Phase::Saving {
                    cancelled: false,
                    ..
                } | Phase::Reopening { cancelled: false }
            ),
            SavedQuerySqlSaveCommand::Recover => {
                self.can_resolve_saved_query_sql_save(identity, true)
            }
            SavedQuerySqlSaveCommand::Release => {
                self.can_resolve_saved_query_sql_save(identity, false)
            }
            SavedQuerySqlSaveCommand::Acknowledge => {
                self.can_acknowledge_saved_query_sql_save(identity)
            }
            SavedQuerySqlSaveCommand::RetryReopen | SavedQuerySqlSaveCommand::ReturnToDraft => {
                status.phase == Phase::ReopenFailed
            }
        };
        if !permitted {
            return None;
        }
        if command == SavedQuerySqlSaveCommand::Cancel {
            let status = self.saved_query_sql_save.as_mut().unwrap();
            match &mut status.phase {
                Phase::Saving { cancelled, .. } | Phase::Reopening { cancelled } => {
                    *cancelled = true
                }
                _ => {}
            }
        }
        Some(Action::SavedQuerySqlSave { identity, command })
    }
}

fn without_report(outcome: EditSaveOutcome) -> EditSaveOutcome {
    match outcome {
        EditSaveOutcome::Retained {
            can_recover,
            can_release,
            ..
        } => EditSaveOutcome::Retained {
            message: String::new(),
            can_recover,
            can_release,
        },
        EditSaveOutcome::Saved(_) => EditSaveOutcome::Saved(String::new()),
        EditSaveOutcome::Cancelled(_) => EditSaveOutcome::Cancelled(String::new()),
        EditSaveOutcome::Failed(_) => EditSaveOutcome::Failed(String::new()),
    }
}

fn outcome_bytes(outcome: &EditSaveOutcome) -> usize {
    match outcome {
        EditSaveOutcome::Retained { message, .. }
        | EditSaveOutcome::Saved(message)
        | EditSaveOutcome::Cancelled(message)
        | EditSaveOutcome::Failed(message) => message.capacity(),
    }
}

fn copy_request(request: &SavedQuerySqlRequest) -> Option<SavedQuerySqlRequest> {
    Some(SavedQuerySqlRequest {
        target: super::SavedQueryTarget {
            document_revision: request.target.document_revision,
            object_id: request.target.object_id,
            name: copy_text(&request.target.name)?,
        },
        draft_revision: request.draft_revision,
        original_sql: match &request.original_sql {
            Some(original) => Some(copy_text(original)?),
            None => None,
        },
        proposed_sql: copy_text(&request.proposed_sql)?,
    })
}

#[cfg(test)]
mod tests;
