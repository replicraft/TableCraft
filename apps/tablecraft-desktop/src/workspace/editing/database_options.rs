//! Explicit database-settings composition; admission and persistence stay runtime-owned.
mod commands;
mod completion;
#[cfg(test)]
mod tests;
use super::{ActiveSave, SaveOwner, cleanup_notice, save_outcome};
use crate::workspace::request_read;
use crate::workspace::{Desktop, PendingOperation};
use std::sync::Arc;
use tablecraft_runtime::{
    Command, DatabaseOptionsInspection, DatabaseOptionsReopen, DatabaseOptionsReview,
    NameAutoCorrectState as NativeState, ReviewedDatabaseOptions, SaveDatabaseOptionsChange,
    SaveOutcome, journal_path,
};
use tablecraft_ui::{
    DatabaseOptionsSaveIdentity as Identity, NameAutoCorrectState as ViewState, SaveCleanupNotice,
};

pub(in crate::workspace) struct DatabaseOptionsHost {
    revision: u64,
    request_id: u64,
    identity: Option<Identity>,
    witness: Option<Arc<DatabaseOptionsReopen>>,
    terminal: Option<SaveOutcome>,
    cleanup: Option<SaveCleanupNotice>,
    reopening: bool,
    cancelled: bool,
}

impl Desktop {
    pub(in crate::workspace) fn inspect_database_options(&mut self, revision: u64) {
        if self.editing.has_save() || !self.state.prepare_database_options_inspection(revision) {
            return;
        }
        let Some(id) = request_read(
            Command::InspectDatabaseOptions {
                document_revision: revision,
            },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            let message = self
                .state
                .localizer
                .text("objects-read-identities-exhausted");
            self.state
                .fail_database_options_inspection(revision, 0, message);
            return;
        };
        if !self.state.bind_database_options_inspection(revision, id) {
            self.worker.cancel();
            self.state.fail_database_options_inspection(
                revision,
                0,
                self.state
                    .localizer
                    .text("database-options-inspection-interrupted"),
            );
            return;
        }
        self.editing.database_options_inspection = Some((revision, id));
        self.generation = id;
        self.pending = PendingOperation::DatabaseOptionsInspection {
            revision,
            request_id: id,
        };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn publish_database_options_inspection(
        &mut self,
        generation: u64,
        inspection: DatabaseOptionsInspection,
    ) {
        let Some((revision, id)) = self.editing.database_options_inspection else {
            return;
        };
        if id != generation {
            return;
        }
        self.editing.database_options_inspection = None;
        let observed = match inspection.name_auto_correct {
            NativeState::Missing => ViewState::Missing,
            NativeState::Absent => ViewState::Absent,
            NativeState::ExplicitOff => ViewState::ExplicitOff,
            NativeState::Other => ViewState::Other,
        };
        if inspection.document_revision != revision
            || !self.state.set_database_options_inspection(
                revision,
                id,
                observed,
                inspection.disable_available,
            )
        {
            self.state.fail_database_options_inspection(
                revision,
                id,
                self.state.localizer.text("objects-open-refused"),
            );
        }
    }

    pub(in crate::workspace) fn fail_database_options_inspection(
        &mut self,
        revision: u64,
        request_id: u64,
        message: String,
    ) {
        if self.editing.database_options_inspection != Some((revision, request_id)) {
            return;
        }
        self.editing.database_options_inspection = None;
        self.state
            .fail_database_options_inspection(revision, request_id, message);
    }

    pub(in crate::workspace) fn retire_database_options_inspection_publication(&mut self) {
        let Some((revision, request_id)) = self.editing.database_options_inspection else {
            return;
        };
        if self.worker.publication_is_current(request_id) {
            return;
        }
        self.fail_database_options_inspection(
            revision,
            request_id,
            self.state
                .localizer
                .text("database-options-inspection-interrupted"),
        );
        if self.generation == request_id {
            self.state.busy = false;
            self.pending = PendingOperation::Read;
        }
    }

    pub(in crate::workspace) fn disable_name_auto_correct(
        &mut self,
        request: tablecraft_ui::DatabaseOptionsCloseRequest,
    ) {
        let revision = request.document_revision;
        if !self.state.database_options_close_request_current(request)
            || self.editing.has_save()
            || self.creating.is_busy()
            || self.file_dialog.is_open()
            || self.recovery.is_some()
            || !self.state.can_disable_name_auto_correct()
            || !self.state.can_rebind_database_options_layout(revision)
        {
            return;
        }
        let Some(id) = request_read(
            Command::ReviewDisableNameAutoCorrect {
                document_revision: revision,
            },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            return;
        };
        if !self.state.begin_database_options_review(revision, id) {
            self.worker.cancel();
            return;
        }
        self.editing.database_options = Some(DatabaseOptionsHost {
            revision,
            request_id: id,
            identity: None,
            witness: None,
            terminal: None,
            cleanup: None,
            reopening: false,
            cancelled: false,
        });
        self.generation = id;
        self.pending = PendingOperation::DatabaseOptionsReview { request_id: id };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn start_reviewed_database_options(
        &mut self,
        generation: u64,
        review: DatabaseOptionsReview,
        capability: ReviewedDatabaseOptions,
    ) {
        let Some(host) = &self.editing.database_options else {
            return;
        };
        if generation != host.request_id
            || generation != self.generation
            || !matches!(self.pending, PendingOperation::DatabaseOptionsReview { request_id } if request_id == generation)
        {
            return;
        }
        let witness = capability.reopen_witness();
        if host.cancelled
            || !self
                .state
                .database_options_review_is_current(host.revision, generation)
            || !self.state.can_rebind_database_options_layout(host.revision)
            || review.document_revision != host.revision
            || review.change != SaveDatabaseOptionsChange::NameAutoCorrectDisabled
            || witness.document_revision() != host.revision
            || witness.source_path() != capability.source_path()
        {
            self.database_options_review_failed(
                generation,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        }
        let revision = host.revision;
        let path = capability.source_path().to_path_buf();
        let bundle = journal_path(&path);
        let Some(prepared) = self
            .state
            .prepare_database_options_save(revision, generation, review.id)
        else {
            self.database_options_review_failed(
                generation,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        };
        match self
            .editing
            .worker
            .start_disable_name_auto_correct(capability, bundle)
        {
            Ok(save_id) => {
                let identity = prepared.bind(save_id);
                self.editing.save = Some(ActiveSave {
                    owner: SaveOwner::DatabaseOptions,
                    id: save_id,
                    path,
                    table: String::new(),
                    offset: 0,
                    report: None,
                    insert_retry: None,
                });
                let host = self.editing.database_options.as_mut().unwrap();
                host.identity = Some(identity);
                host.witness = Some(witness);
                self.state.busy = true;
            }
            Err(error) => {
                drop(prepared);
                self.state.error = Some(error.to_string());
                self.state.database_options_restore_settings();
                drop(error.into_reviewed());
                self.editing.database_options = None;
                self.state.busy = false;
            }
        }
    }

    pub(in crate::workspace) fn database_options_review_failed(
        &mut self,
        request_id: u64,
        message: String,
    ) {
        let Some(host) = self
            .editing
            .database_options
            .as_ref()
            .filter(|host| host.request_id == request_id && host.identity.is_none())
        else {
            return;
        };
        self.state
            .release_database_options_review(host.revision, request_id, Some(message));
        self.editing.database_options = None;
        self.state.database_options_restore_settings();
        self.state.busy = false;
        self.pending = PendingOperation::Read;
    }

    pub(in crate::workspace) fn cancel_database_options_review(&mut self, request_id: u64) {
        let Some(host) = self
            .editing
            .database_options
            .as_ref()
            .filter(|host| host.request_id == request_id && host.identity.is_none())
        else {
            return;
        };
        let revision = host.revision;
        // This readonly cancellation revokes publication immediately. No native
        // actor exists and no terminal response can release the presentation.
        self.worker.cancel();
        self.state
            .release_database_options_review(revision, request_id, None);
        self.state.database_options_restore_settings();
        self.editing.database_options = None;
        self.state.busy = false;
        self.pending = PendingOperation::Read;
    }
}
