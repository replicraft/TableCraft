use super::{TableDropRequest, TableDropSaveIdentity, model::Phase};
use crate::workspace::{
    EditSaveOutcome, SaveCleanupNotice, State,
    schema_save::{Status, StatusPhase, outcome_bytes},
};

/// Request controls stay tied to the actual reviewed deletion and save actor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableDropSaveCommand {
    Cancel,
    Recover,
    Release,
    Acknowledge,
    RetryReopen,
    ReturnToCatalog,
}

impl State {
    /// Bind an already accepted actor without allocating or changing the catalog.
    pub fn begin_table_drop_save(
        &mut self,
        request: &TableDropRequest,
        identity: TableDropSaveIdentity,
    ) -> bool {
        if identity.review_id == 0
            || identity.save_id == 0
            || !self.table_drop_review_matches(request, identity.request_id)
        {
            return false;
        }
        let Some(session) = self.table_drop.session.as_mut() else {
            return false;
        };
        session.phase = Phase::Owned(identity);
        session.status = Some(Status {
            identity,
            phase: StatusPhase::Saving {
                cancelled: false,
                resolving: false,
            },
            outcome: None,
            error: None,
            omitted: false,
        });
        true
    }

    /// Compare all request, native review and actor identities before a callback.
    pub fn matches_table_drop_save(
        &self,
        request: &TableDropRequest,
        identity: TableDropSaveIdentity,
    ) -> bool {
        self.table_drop.matches(request)
            && self.table_drop.session.as_ref().is_some_and(|session| {
                session.phase == Phase::Owned(identity)
                    && session
                        .status
                        .as_ref()
                        .is_some_and(|status| status.identity == identity)
            })
    }

    /// Store a terminal report without implying publication or dropping resource owners.
    pub fn set_table_drop_save_outcome(
        &mut self,
        identity: TableDropSaveIdentity,
        outcome: EditSaveOutcome,
    ) -> bool {
        if !self.table_drop.session.as_ref().is_some_and(|session| {
            session
                .status
                .as_ref()
                .is_some_and(|status| status.accepts_outcome(identity))
        }) {
            return false;
        }
        let admitted = self.admit_object_bytes(Some(outcome_bytes(&outcome)));
        let Some(status) = self
            .table_drop
            .session
            .as_mut()
            .and_then(|session| session.status.as_mut())
        else {
            return false;
        };
        status.record_outcome(outcome, admitted);
        true
    }

    /// Real actor teardown certifies neither image and revokes future mutation.
    pub fn set_table_drop_actor_lost(
        &mut self,
        identity: TableDropSaveIdentity,
        message: String,
    ) -> bool {
        if !self.table_drop.session.as_ref().is_some_and(|session| {
            session.status.as_ref().is_some_and(|status| {
                status.identity == identity
                    && matches!(
                        status.phase,
                        StatusPhase::Saving { .. } | StatusPhase::Finished
                    )
            })
        }) {
            return false;
        }
        let outcome = EditSaveOutcome::Failed(message);
        let admitted = self.admit_object_bytes(Some(outcome_bytes(&outcome)));
        let Some(session) = self.table_drop.session.as_mut() else {
            return false;
        };
        session.uncertain = true;
        let Some(status) = session.status.as_mut() else {
            return false;
        };
        status.record_outcome(outcome, admitted);
        self.mark_native_save_uncertain();
        true
    }

    /// Return only a retained recovery/release permission reported by this actor.
    pub fn can_resolve_table_drop_save(
        &self,
        identity: TableDropSaveIdentity,
        recover: bool,
    ) -> bool {
        self.table_drop.session.as_ref().is_some_and(|session| {
            !session.uncertain
                && session
                    .status
                    .as_ref()
                    .is_some_and(|status| status.can_resolve(identity, recover))
        })
    }

    /// Bind an accepted recovery/release command to this actor's retained permission.
    /// The host keeps the actual session and rechecks its consuming command.
    pub fn begin_table_drop_resolution(
        &mut self,
        identity: TableDropSaveIdentity,
        recover: bool,
    ) -> bool {
        if !self.can_resolve_table_drop_save(identity, recover) {
            return false;
        }
        let Some(status) = self
            .table_drop
            .session
            .as_mut()
            .and_then(|session| session.status.as_mut())
        else {
            return false;
        };
        status.phase = StatusPhase::Saving {
            cancelled: false,
            resolving: true,
        };
        status.error = None;
        true
    }

    /// Bind the new actor ID only after the host accepted this exact recovery or release.
    /// No allocation occurs between accepted worker ownership and status installation.
    pub fn bind_table_drop_resolution(
        &mut self,
        identity: TableDropSaveIdentity,
        recover: bool,
        next_save_id: u64,
    ) -> Option<TableDropSaveIdentity> {
        if next_save_id == 0 || !self.can_resolve_table_drop_save(identity, recover) {
            return None;
        }
        let next = TableDropSaveIdentity {
            save_id: next_save_id,
            ..identity
        };
        let session = self.table_drop.session.as_mut()?;
        let status = session.status.as_mut()?;
        session.phase = Phase::Owned(next);
        status.identity = next;
        status.phase = StatusPhase::Saving {
            cancelled: false,
            resolving: true,
        };
        status.error = None;
        Some(next)
    }

    /// Acknowledgement itself grants no catalog refresh or native-write permission.
    pub fn can_acknowledge_table_drop_save(&self, identity: TableDropSaveIdentity) -> bool {
        self.table_drop.session.as_ref().is_some_and(|session| {
            session
                .status
                .as_ref()
                .is_some_and(|status| status.can_acknowledge(identity))
        })
    }

    /// Host prerequisite: actual actor ACK/release established known AFTER.
    /// The later operation-specific full-image read must still validate it.
    pub fn begin_table_drop_reopen(&mut self, identity: TableDropSaveIdentity) -> bool {
        let Some(session) = self.table_drop.session.as_mut() else {
            return false;
        };
        if session.uncertain {
            return false;
        }
        session
            .status
            .as_mut()
            .is_some_and(|status| status.begin_reopen(identity))
    }

    /// Preserve the actual saved outcome when the complete AFTER witness read fails.
    pub fn fail_table_drop_reopen(
        &mut self,
        identity: TableDropSaveIdentity,
        message: String,
    ) -> bool {
        if !self.table_drop_reopening(identity) {
            return false;
        }
        let admitted = self.admit_object_bytes(Some(message.capacity()));
        let Some(status) = self
            .table_drop
            .session
            .as_mut()
            .and_then(|session| session.status.as_mut())
        else {
            return false;
        };
        status.record_reopen_failure(message, admitted);
        true
    }

    /// Cancel only once; a requested cancellation cannot invent rollback.
    pub fn cancel_table_drop_operation(&mut self, identity: TableDropSaveIdentity) -> bool {
        self.table_drop
            .session
            .as_mut()
            .and_then(|session| session.status.as_mut())
            .is_some_and(|status| status.cancel(identity))
    }

    /// The exact accepted AFTER witness remains the only possible retry source.
    pub fn can_retry_table_drop_reopen(&self, identity: TableDropSaveIdentity) -> bool {
        self.table_drop.session.as_ref().is_some_and(|session| {
            !session.uncertain
                && session
                    .status
                    .as_ref()
                    .is_some_and(|status| status.can_retry_reopen(identity))
        })
    }

    /// Store cleanup facts separately from outcome and refresh authority.
    pub fn set_table_drop_cleanup(
        &mut self,
        identity: TableDropSaveIdentity,
        cleanup: Option<SaveCleanupNotice>,
    ) -> bool {
        let Some(session) = self.table_drop.session.as_mut() else {
            return false;
        };
        if session.phase != Phase::Owned(identity) {
            return false;
        }
        session.cleanup = cleanup;
        true
    }

    /// Host prerequisite: known original/no-write ACK released the actual actor.
    /// Retrying deletion requires a new readonly observation and consuming review.
    pub fn release_table_drop_save(&mut self, identity: TableDropSaveIdentity) -> bool {
        if !self.can_acknowledge_table_drop_save(identity) {
            return false;
        }
        let Some(session) = self.table_drop.session.as_mut() else {
            return false;
        };
        if session.uncertain {
            return false;
        }
        session.phase = Phase::Confirming;
        session.observation = super::model::Observation::Interrupted;
        session.status = None;
        true
    }

    /// Host prerequisite: the exact uncertain actor has been acknowledged.
    /// Keep its warning and revision guard even after the modal is hidden.
    pub fn acknowledge_uncertain_table_drop_save(
        &mut self,
        identity: TableDropSaveIdentity,
    ) -> bool {
        if !self.can_acknowledge_table_drop_save(identity)
            || !self
                .table_drop
                .session
                .as_ref()
                .is_some_and(|session| session.uncertain)
        {
            return false;
        }
        let Some(session) = self.table_drop.session.as_mut() else {
            return false;
        };
        session.visible = false;
        self.mark_native_save_uncertain();
        true
    }

    /// Leave failed publication details visible as a retained warning, not a rollback.
    pub fn return_from_table_drop_reopen(&mut self, identity: TableDropSaveIdentity) -> bool {
        if !self.can_retry_table_drop_reopen(identity) {
            return false;
        }
        let Some(session) = self.table_drop.session.as_mut() else {
            return false;
        };
        session.visible = false;
        true
    }

    pub(in crate::workspace) fn table_drop_reopening(
        &self,
        identity: TableDropSaveIdentity,
    ) -> bool {
        self.table_drop.session.as_ref().is_some_and(|session| {
            session
                .status
                .as_ref()
                .is_some_and(|status| status.reopening(identity))
        })
    }

    pub(in crate::workspace) fn finish_table_drop_publication(&mut self) {
        self.table_drop.session = None;
    }

    pub(in crate::workspace) fn table_drop_source_stale(&self) -> bool {
        self.table_drop.session.as_ref().is_some_and(|session| {
            self.database.as_ref().is_some_and(|database| {
                database.document_revision == session.request.target.document_revision
                    && (session.uncertain
                        || session.status.as_ref().is_some_and(|status| {
                            matches!(
                                status.phase,
                                StatusPhase::Reopening { .. } | StatusPhase::ReopenFailed
                            )
                        }))
            })
        })
    }

    pub(in crate::workspace) fn retire_table_drop_warning(&mut self, revision: Option<u64>) {
        if self.table_drop.session.as_ref().is_some_and(|session| {
            !session.visible && Some(session.request.target.document_revision) != revision
        }) {
            self.table_drop.session = None;
        }
    }
}
