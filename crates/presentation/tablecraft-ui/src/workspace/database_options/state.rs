//! Exact revision/actor lifecycle; only the host can acknowledge native outcomes.
use super::{
    DatabaseOptionsSaveIdentity as Identity, Model, NameAutoCorrectState, Phase,
    PreparedDatabaseOptionsSave, outcome_bytes,
};
use crate::workspace::{EditSaveOutcome, SaveCleanupNotice, State};

impl State {
    pub fn database_options_observation(&self) -> Option<NameAutoCorrectState> {
        let revision = self.database.as_ref()?.document_revision;
        (self.database_options.revision == Some(revision))
            .then_some(self.database_options.observed?.0)
    }

    pub fn database_options_pending(&self) -> bool {
        !matches!(
            self.database_options.phase,
            Phase::Idle | Phase::Warning { .. }
        )
    }

    pub fn database_options_source_stale(&self) -> bool {
        matches!(self.database_options.phase, Phase::Warning { identity, .. }
            if self.database.as_ref().is_some_and(|database| database.document_revision == identity.document_revision))
    }

    pub fn needs_database_options_inspection(&self) -> bool {
        !self.busy
            && !self.database_options_pending()
            && !self.database_options_source_stale()
            && self.database.as_ref().is_some_and(|database| {
                self.database_options.revision != Some(database.document_revision)
            })
    }

    pub fn begin_database_options_inspection(&mut self, revision: u64, request_id: u64) -> bool {
        if !self.needs_database_options_inspection()
            || self
                .database
                .as_ref()
                .is_none_or(|database| database.document_revision != revision)
        {
            return false;
        }
        self.database_options = Model {
            revision: Some(revision),
            phase: Phase::Inspecting { request_id },
            ..Model::default()
        };
        true
    }

    pub fn set_database_options_inspection(
        &mut self,
        revision: u64,
        request_id: u64,
        observed: NameAutoCorrectState,
        available: bool,
    ) -> bool {
        if self.database_options.revision != Some(revision)
            || self.database_options.phase != (Phase::Inspecting { request_id })
            || self
                .database
                .as_ref()
                .is_none_or(|database| database.document_revision != revision)
        {
            return false;
        }
        self.database_options.observed = Some((observed, available));
        self.database_options.phase = Phase::Idle;
        true
    }

    pub fn fail_database_options_inspection(
        &mut self,
        revision: u64,
        request_id: u64,
        message: String,
    ) -> bool {
        if self.database_options.revision != Some(revision)
            || self.database_options.phase != (Phase::Inspecting { request_id })
        {
            return false;
        }
        let admitted = self.admit_object_bytes(Some(message.capacity()));
        self.database_options.error = Some(if admitted { message } else { String::new() });
        self.database_options.omitted |= !admitted;
        self.database_options.phase = Phase::Idle;
        true
    }

    pub fn retry_database_options_inspection(&mut self) -> bool {
        if self.busy || self.database_options_pending() || self.database_options_source_stale() {
            return false;
        }
        self.database_options = Model::default();
        true
    }

    pub fn can_disable_name_auto_correct(&self) -> bool {
        let Some(database) = &self.database else {
            return false;
        };
        self.database_options_available
            && !self.busy
            && !self.native_source_stale()
            && self.database_options.revision == Some(database.document_revision)
            && self.database_options.phase == Phase::Idle
            && self
                .database_options
                .observed
                .is_some_and(|(_, available)| available)
            && self.can_rebind_database_options_layout(database.document_revision)
    }

    pub fn begin_database_options_review(&mut self, revision: u64, request_id: u64) -> bool {
        if !self.database_options_close_graph_current()
            || !self.can_disable_name_auto_correct()
            || self
                .database
                .as_ref()
                .is_none_or(|database| database.document_revision != revision)
        {
            return false;
        }
        self.database_options.phase = Phase::Reviewing {
            request_id,
            cancelled: false,
        };
        self.database_options.error = None;
        true
    }

    pub fn database_options_review_is_current(&self, revision: u64, request_id: u64) -> bool {
        self.database_options_close_graph_current()
            && self.database_options.revision == Some(revision)
            && matches!(self.database_options.phase, Phase::Reviewing { request_id: id, cancelled: false } if id == request_id)
            && self
                .database
                .as_ref()
                .is_some_and(|database| database.document_revision == revision)
    }

    pub fn cancel_database_options_review(&mut self, request_id: u64) -> bool {
        if let Phase::Reviewing {
            request_id: id,
            cancelled,
        } = &mut self.database_options.phase
            && *id == request_id
            && !*cancelled
        {
            *cancelled = true;
            return true;
        }
        false
    }

    pub fn release_database_options_review(
        &mut self,
        revision: u64,
        request_id: u64,
        message: Option<String>,
    ) -> bool {
        if self.database_options.revision != Some(revision)
            || !matches!(self.database_options.phase, Phase::Reviewing { request_id: id, .. } if id == request_id)
        {
            return false;
        }
        let admitted = self.admit_object_bytes(Some(message.as_ref().map_or(0, String::capacity)));
        self.database_options.error =
            message.map(|message| if admitted { message } else { String::new() });
        self.database_options.omitted |= !admitted;
        self.database_options.phase = Phase::Idle;
        true
    }

    pub fn prepare_database_options_save(
        &mut self,
        revision: u64,
        request_id: u64,
        review_id: u64,
    ) -> Option<PreparedDatabaseOptionsSave<'_>> {
        if !self.database_options_review_is_current(revision, request_id)
            || !self.can_rebind_database_options_layout(revision)
        {
            return None;
        }
        self.database_options.phase = Phase::Reviewed {
            request_id,
            review_id,
        };
        Some(PreparedDatabaseOptionsSave {
            model: &mut self.database_options,
            document_revision: revision,
            request_id,
            review_id,
            bound: false,
            restore: Phase::Idle,
            resolving: false,
        })
    }

    pub fn matches_database_options_save(&self, identity: Identity) -> bool {
        self.database_options.identity() == Some(identity)
            && self.database_options.revision == Some(identity.document_revision)
    }

    pub fn database_options_save_status(&self) -> Option<(Identity, Option<&EditSaveOutcome>)> {
        Some((
            self.database_options.identity()?,
            self.database_options.outcome.as_ref(),
        ))
    }

    pub fn set_database_options_save_outcome(
        &mut self,
        identity: Identity,
        mut outcome: EditSaveOutcome,
    ) -> bool {
        if !self.matches_database_options_save(identity)
            || !matches!(self.database_options.phase, Phase::Saving { .. })
        {
            return false;
        }
        let admitted = self.admit_object_bytes(Some(outcome_bytes(&outcome)));
        if !admitted {
            match &mut outcome {
                EditSaveOutcome::Saved(text)
                | EditSaveOutcome::Failed(text)
                | EditSaveOutcome::Cancelled(text)
                | EditSaveOutcome::Retained { message: text, .. } => *text = String::new(),
            }
        }
        self.database_options.outcome = Some(outcome);
        self.database_options.omitted |= !admitted;
        self.database_options.phase = Phase::Finished { identity };
        true
    }

    pub fn can_resolve_database_options_save(&self, identity: Identity, recover: bool) -> bool {
        self.matches_database_options_save(identity)
            && matches!(self.database_options.phase, Phase::Finished { .. })
            && self.database_options.outcome.as_ref().is_some_and(|outcome| {
                matches!(outcome, EditSaveOutcome::Retained { can_recover, can_release, .. } if if recover { *can_recover } else { *can_release })
            })
    }

    pub fn can_acknowledge_database_options_save(&self, identity: Identity) -> bool {
        self.matches_database_options_save(identity)
            && matches!(self.database_options.phase, Phase::Finished { .. })
            && self
                .database_options
                .outcome
                .as_ref()
                .is_some_and(|outcome| !matches!(outcome, EditSaveOutcome::Retained { .. }))
    }

    pub fn begin_database_options_reopen(&mut self, identity: Identity) -> bool {
        if !self.matches_database_options_save(identity)
            || !matches!(
                self.database_options.phase,
                Phase::Finished { .. } | Phase::ReopenFailed { .. }
            )
        {
            return false;
        }
        self.database_options.phase = Phase::Reopening {
            identity,
            cancelled: false,
        };
        self.database_options.error = None;
        true
    }

    pub fn fail_database_options_reopen(&mut self, identity: Identity, message: String) -> bool {
        if !self.matches_database_options_save(identity)
            || !matches!(self.database_options.phase, Phase::Reopening { .. })
        {
            return false;
        }
        let admitted = self.admit_object_bytes(Some(message.capacity()));
        self.database_options.error = Some(if admitted { message } else { String::new() });
        self.database_options.omitted |= !admitted;
        self.database_options.phase = Phase::ReopenFailed { identity };
        true
    }

    pub fn cancel_database_options_operation(&mut self, identity: Identity) -> bool {
        if !self.matches_database_options_save(identity) {
            return false;
        }
        match &mut self.database_options.phase {
            Phase::Saving { cancelled, .. } | Phase::Reopening { cancelled, .. } if !*cancelled => {
                *cancelled = true;
                true
            }
            _ => false,
        }
    }

    /// Caller has acknowledged original/no-write, or completed full-witness publication.
    pub fn clear_database_options_save(&mut self, identity: Identity) -> bool {
        if !self.matches_database_options_save(identity) {
            return false;
        }
        self.database_options = Model::default();
        true
    }

    /// Caller has acknowledged known AFTER but failed its complete witness read.
    pub fn return_from_database_options_reopen(
        &mut self,
        identity: Identity,
        cleanup: Option<SaveCleanupNotice>,
    ) -> bool {
        if !self.matches_database_options_save(identity)
            || !matches!(self.database_options.phase, Phase::ReopenFailed { .. })
        {
            return false;
        }
        self.database_options.cleanup = cleanup;
        self.database_options.phase = Phase::Warning {
            identity,
            open: true,
        };
        true
    }
}

impl State {
    pub(in crate::workspace) fn database_options_reopen_pending(&self, identity: Identity) -> bool {
        self.matches_database_options_save(identity)
            && matches!(self.database_options.phase, Phase::Reopening { .. })
    }

    pub(in crate::workspace) fn retire_database_options_warning(&mut self, revision: Option<u64>) {
        if matches!(self.database_options.phase, Phase::Warning { .. })
            && self.database_options.revision != revision
        {
            self.database_options = Model::default();
        }
    }
}

impl State {
    pub fn prepare_database_options_resolution(
        &mut self,
        identity: Identity,
        recover: bool,
    ) -> Option<PreparedDatabaseOptionsSave<'_>> {
        if !self.can_resolve_database_options_save(identity, recover) {
            return None;
        }
        let restore = self.database_options.phase;
        Some(PreparedDatabaseOptionsSave {
            model: &mut self.database_options,
            document_revision: identity.document_revision,
            request_id: identity.request_id,
            review_id: identity.review_id,
            bound: false,
            restore,
            resolving: true,
        })
    }

    pub fn prepare_database_options_inspection(&mut self, revision: u64) -> bool {
        self.begin_database_options_inspection(revision, 0)
    }

    pub fn bind_database_options_inspection(&mut self, revision: u64, request_id: u64) -> bool {
        if self.database_options.revision != Some(revision)
            || self.database_options.phase != (Phase::Inspecting { request_id: 0 })
        {
            return false;
        }
        self.database_options.phase = Phase::Inspecting { request_id };
        true
    }
}
