//! Consuming save and terminal resource handling for the nominal creation owner.
use super::super::{SaveOwner, cleanup_notice, save_outcome};
use crate::workspace::{Desktop, PendingOperation};
use tablecraft_runtime::{SaveCompletion, SaveOperation, SaveOutcome, SaveReport};
use tablecraft_ui::{
    QueryCreationSaveCommand, QueryCreationSaveIdentity, QueryCreationSaveOperation,
};

impl Desktop {
    pub(in crate::workspace) fn query_creation_save_report(&mut self, report: SaveReport) {
        let Some(status) = self
            .editing
            .query_creation
            .as_ref()
            .and_then(|host| host.status)
        else {
            return;
        };
        if !matches!(status.operation, QueryCreationSaveOperation::NativeSave { save_id } if save_id == report.id)
        {
            return;
        }
        if !self.state.set_query_creation_save_outcome(
            status,
            save_outcome(&self.state.localizer, report.outcome),
        ) {
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return;
        }
        let ordinary = self
            .editing
            .query_creation
            .as_ref()
            .is_some_and(|host| !host.cancellation_requested)
            && self.editing.save.as_ref().is_some_and(|save| {
                save.id == report.id
                    && matches!(save.owner, SaveOwner::QueryCreation)
                    && save.report == Some(report)
            })
            && report.operation == SaveOperation::Apply
            && matches!(report.outcome, SaveOutcome::Saved { receipt } if receipt.completion == SaveCompletion::Applied)
            && report
                .outcome
                .query_creation_reopen_facts()
                .is_ok_and(|facts| facts.replacement);
        if ordinary {
            self.acknowledge_query_creation(report.id);
        }
    }

    pub(in crate::workspace) fn note_query_creation_save_cancellation(&mut self, id: u64) {
        if self
            .editing
            .save
            .as_ref()
            .is_some_and(|save| save.id == id && matches!(save.owner, SaveOwner::QueryCreation))
            && let Some(host) = &mut self.editing.query_creation
        {
            host.cancellation_requested = true;
        }
    }

    pub(in crate::workspace) fn query_creation_save_action(
        &mut self,
        identity: QueryCreationSaveIdentity,
        command: QueryCreationSaveCommand,
    ) {
        if !self
            .editing
            .query_creation
            .as_ref()
            .is_some_and(|host| host.status == Some(identity))
            || !self.state.matches_query_creation_save(identity)
        {
            return;
        }
        match command {
            QueryCreationSaveCommand::Cancel => {
                if let Some(save) = &self.editing.save {
                    self.cancel_save(save.id);
                } else {
                    self.cancel_query_creation_operation();
                }
            }
            QueryCreationSaveCommand::Recover | QueryCreationSaveCommand::Release => {
                if let QueryCreationSaveOperation::NativeSave { save_id } = identity.operation {
                    self.resolve_save(save_id, command == QueryCreationSaveCommand::Recover);
                }
            }
            QueryCreationSaveCommand::Acknowledge => {
                if self.state.can_acknowledge_query_creation_save(identity)
                    && let QueryCreationSaveOperation::NativeSave { save_id } = identity.operation
                {
                    self.acknowledge_query_creation(save_id);
                }
            }
            QueryCreationSaveCommand::RetryReopen => self.reopen_query_creation(),
            QueryCreationSaveCommand::ReturnToDraft => {
                self.keep_query_creation_draft_after_reopen_failure();
            }
        }
    }

    pub(in crate::workspace) fn can_resolve_query_creation_save(
        &self,
        id: u64,
        recover: bool,
    ) -> bool {
        let Some(status) = self
            .editing
            .query_creation
            .as_ref()
            .and_then(|host| host.status)
        else {
            return false;
        };
        if !self.state.can_resolve_query_creation_save(status, recover) {
            return false;
        }
        let Some(save) = self
            .editing
            .save
            .as_ref()
            .filter(|save| save.id == id && matches!(save.owner, SaveOwner::QueryCreation))
        else {
            return false;
        };
        let Some(SaveReport {
            outcome: SaveOutcome::Failed(failure),
            ..
        }) = save.report
        else {
            return false;
        };
        if recover {
            failure.can_recover
        } else {
            failure.can_release
        }
    }

    pub(in crate::workspace) fn query_creation_resolution_started(&mut self, next: u64) {
        let Some(host) = &mut self.editing.query_creation else {
            return;
        };
        let Some(status) = host.status else {
            return;
        };
        match self
            .state
            .begin_query_creation_save_resolution(status, next)
        {
            Some(next) => host.status = Some(next),
            None => self.state.error = Some(self.state.localizer.text("objects-open-refused")),
        }
        self.state.busy = true;
    }

    /// Uncertainty retires only the real actor and pending presentation. It never
    /// turns a new query into a published saved object or an original/no-write fact.
    pub(in crate::workspace) fn acknowledge_uncertain_query_creation(&mut self, id: u64) -> bool {
        let Some(host) = &self.editing.query_creation else {
            return false;
        };
        let (Some(review_id), Some(identity)) = (host.review_id, host.status) else {
            return false;
        };
        if !self
            .state
            .matches_query_creation_review(host.request_id, &host.request, review_id)
            || !self.state.can_acknowledge_query_creation_save(identity)
            || !self.editing.worker.acknowledge(id)
        {
            return false;
        }
        self.state
            .release_query_creation_review(host.request_id, &host.request, review_id, None);
        self.state.clear_query_creation_save(identity);
        self.editing.query_creation = None;
        true
    }

    pub(in crate::workspace) fn acknowledge_query_creation(&mut self, id: u64) {
        if self.acknowledge_uncertain_save(id) {
            return;
        }
        if self.editing.save.is_none() {
            if self.editing.query_creation.as_ref().is_some_and(|host| matches!(host.status.map(|status| status.operation), Some(QueryCreationSaveOperation::NativeSave { save_id }) if save_id == id)) {
                self.keep_query_creation_draft_after_reopen_failure();
            }
            return;
        }
        let Some(save) = self
            .editing
            .save
            .as_ref()
            .filter(|save| save.id == id && matches!(save.owner, SaveOwner::QueryCreation))
        else {
            return;
        };
        let Some(report) = save.report else {
            return;
        };
        let Some(host) = &self.editing.query_creation else {
            return;
        };
        let (Some(review_id), Some(status)) = (host.review_id, host.status) else {
            return;
        };
        if !self
            .state
            .matches_query_creation_review(host.request_id, &host.request, review_id)
            || !self.state.can_acknowledge_query_creation_save(status)
        {
            return;
        }
        let facts = match report.outcome.query_creation_reopen_facts() {
            Ok(facts) => facts,
            Err(error) => {
                self.state.error = Some(error.to_string());
                return;
            }
        };
        if facts.replacement && host.witness.is_none() {
            return;
        }
        if !self.editing.worker.acknowledge(id) {
            return;
        }
        self.editing.save = None;
        let Some(host) = self.editing.query_creation.as_mut() else {
            self.state.error = Some(self.state.localizer.text("objects-open-refused"));
            return;
        };
        host.terminal = Some(report.outcome);
        host.cleanup = facts.completed.and_then(cleanup_notice);
        if facts.replacement {
            self.reopen_query_creation();
        } else if self.state.release_query_creation_review(
            host.request_id,
            &host.request,
            review_id,
            None,
        ) {
            if let Some(status) = host.status {
                self.state.clear_query_creation_save(status);
            }
            self.editing.query_creation = None;
            self.state.busy = false;
            self.pending = PendingOperation::Read;
        }
    }
}
