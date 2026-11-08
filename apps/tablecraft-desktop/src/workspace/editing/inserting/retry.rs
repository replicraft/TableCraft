//! Route explicit retry through runtime-owned outcome evidence and guarded reads.
use crate::workspace::{Desktop, PendingOperation, objects::Sessions};
use std::sync::Arc;
use tablecraft_runtime::{Command, InsertRetryEvidence, InsertRetryWitness};
use tablecraft_ui::Database;

pub(in crate::workspace::editing) struct InsertRetryContext {
    pub(in crate::workspace::editing) witness: Arc<InsertRetryWitness>,
    pub(in crate::workspace::editing) evidence: InsertRetryEvidence,
}

pub(in crate::workspace::editing) struct RetainedInsertRetry {
    pub(in crate::workspace::editing) save_id: u64,
    pub(in crate::workspace::editing) context: InsertRetryContext,
}

impl Desktop {
    pub(in crate::workspace) fn discard_insert_retry(&mut self, save_id: u64) {
        if !self.editing.has_save() && self.state.discard_retained_insert(save_id) {
            self.editing.insert_retry = None;
        }
    }

    pub(in crate::workspace) fn retry_insert(&mut self, save_id: u64) {
        if self.editing.has_save()
            || self.creating.is_busy()
            || !self.state.can_retry_insert(save_id)
        {
            return;
        }
        let Some(retry) = self.editing.insert_retry.as_ref().filter(|retry| {
            retry.save_id == save_id && retry.context.evidence.can_retry_original()
        }) else {
            return;
        };
        let request_id = self.worker.request(Command::OpenInsertOriginal {
            witness: Arc::clone(&retry.context.witness),
        });
        if !self.state.begin_insert_retry(save_id, request_id) {
            self.worker.cancel();
            return;
        }
        self.generation = request_id;
        self.pending = PendingOperation::InsertRetry {
            request_id,
            save_id,
        };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn cancel_insert_retry(&mut self, request_id: u64) {
        if !matches!(&self.pending, PendingOperation::InsertRetry { request_id: expected, .. } if *expected == request_id)
            || self.generation != request_id
        {
            return;
        }
        self.worker.cancel();
        self.state.busy = false;
        self.pending = PendingOperation::Read;
        self.state.fail_insert_retry(
            request_id,
            self.state.localizer.text("shell-operation-cancelled"),
        );
    }

    pub(in crate::workspace) fn finish_insert_retry(
        &mut self,
        request_id: u64,
        save_id: u64,
        database: Database,
        table_creation_available: bool,
        table_design_available: bool,
    ) {
        if !self
            .editing
            .insert_retry
            .as_ref()
            .is_some_and(|retry| retry.save_id == save_id)
        {
            return;
        }
        let Some(target) = self.state.complete_insert_retry(request_id, database) else {
            self.state.fail_insert_retry(
                request_id,
                self.state.localizer.text("insertion-stale-draft"),
            );
            return;
        };
        self.state
            .set_table_creation_available(target.document_revision, table_creation_available);
        self.state
            .set_table_design_available(target.document_revision, table_design_available);
        self.filter = None;
        self.objects = Sessions::default();
        self.open_object(target);
    }
}
