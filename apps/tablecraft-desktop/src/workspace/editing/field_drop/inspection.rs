//! Recorded readonly publications never become destructive write authority.
use crate::workspace::{Desktop, PendingOperation, read_requests};
use tablecraft_runtime::{Command, Error, FieldDropSourceInspection, Response};
use tablecraft_ui::{FieldDropTarget, ObjectKind};

impl Desktop {
    pub(in crate::workspace) fn inspect_field_drop_source(&mut self, target: FieldDropTarget) {
        if self.editing.has_save() {
            return;
        }
        let Some(prepared) = self.state.prepare_field_drop_inspection(&target) else {
            return;
        };
        let revision = target.tab.target.document_revision;
        let ObjectKind::Table { name } = target.tab.target.kind else {
            return;
        };
        let issued = self.worker.request(Command::InspectFieldDropSource {
            document_revision: revision,
            table: name,
            column: target.column,
            native_number: target.native_number,
        });
        let request_id = match issued {
            Ok(id) => id,
            Err(_) => {
                drop(prepared);
                read_requests::exhausted(&self.worker, &mut self.state);
                self.read_request_rejected();
                return;
            }
        };
        prepared.bind(request_id);
        self.generation = request_id;
        self.pending = PendingOperation::FieldDropInspection { request_id };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn publish_field_drop_inspection(
        &mut self,
        generation: u64,
        inspection: FieldDropSourceInspection,
    ) {
        if !matches!(self.pending, PendingOperation::FieldDropInspection { request_id } if generation == request_id)
        {
            return;
        }
        self.complete_recorded_field_drop_inspection(generation, inspection);
    }

    fn complete_recorded_field_drop_inspection(
        &mut self,
        generation: u64,
        inspection: FieldDropSourceInspection,
    ) {
        if !self.state.set_field_drop_inspection_by_id(
            generation,
            inspection.document_revision,
            &inspection.table,
            &inspection.column,
            inspection.native_number,
            inspection.source_eligible,
        ) {
            self.state.fail_field_drop_inspection_by_id(
                generation,
                self.state.localizer.text("objects-open-refused"),
            );
        }
    }

    pub(in crate::workspace) fn field_drop_inspection_failed(
        &mut self,
        generation: u64,
        message: String,
    ) {
        self.state
            .fail_field_drop_inspection_by_id(generation, message);
    }

    pub(in crate::workspace) fn retire_field_drop_inspection(&mut self) {
        let Some(request_id) = self.state.pending_field_drop_inspection_id() else {
            return;
        };
        if !self.worker.publication_is_current(request_id) {
            self.state.interrupt_field_drop_inspection_id(request_id);
        }
    }

    pub(in crate::workspace) fn finish_superseded_field_drop_inspection(
        &mut self,
        generation: u64,
        result: Result<Response, Error>,
    ) {
        if self.state.pending_field_drop_inspection_id() != Some(generation) {
            return;
        }
        if !self.worker.publication_is_current(generation) {
            self.state.interrupt_field_drop_inspection_id(generation);
            return;
        }
        match result {
            Ok(Response::FieldDropSourceInspected { inspection }) => {
                self.complete_recorded_field_drop_inspection(generation, inspection)
            }
            Err(error) => self.field_drop_inspection_failed(generation, error.to_string()),
            Ok(_) => self.field_drop_inspection_failed(
                generation,
                self.state.localizer.text("objects-open-refused"),
            ),
        }
    }

    /// No may already have removed the clean model owner. Bind cancellation to
    /// the actual pending publication before touching the shared readonly worker.
    pub(in crate::workspace) fn cancel_field_drop_inspection(&mut self, request_id: u64) {
        if self.generation != request_id
            || !matches!(self.pending, PendingOperation::FieldDropInspection { request_id: current } if current == request_id)
        {
            return;
        }
        if self.worker.publication_is_current(request_id) {
            self.worker.cancel();
        }
        self.state.interrupt_field_drop_inspection_id(request_id);
        self.pending = PendingOperation::Read;
        self.state.busy = false;
    }
}
