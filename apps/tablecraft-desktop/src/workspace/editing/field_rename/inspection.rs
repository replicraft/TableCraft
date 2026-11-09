//! Exact recorded readonly inspection; no metadata fact becomes save authority.
use crate::workspace::{Desktop, PendingOperation};
use tablecraft_runtime::{Command, Error, FieldRenameSourceInspection, Response};
use tablecraft_ui::{FieldRenameTarget, ObjectKind};

impl Desktop {
    pub(in crate::workspace) fn inspect_field_rename_source(&mut self, target: FieldRenameTarget) {
        if self.editing.has_save() {
            return;
        }
        let Some(prepared) = self.state.prepare_field_rename_inspection(&target) else {
            return;
        };
        let revision = target.tab.target.document_revision;
        let ObjectKind::Table { name } = target.tab.target.kind else {
            return;
        };
        let request_id = self.worker.request(Command::InspectFieldRenameSource {
            document_revision: revision,
            table: name,
            column: target.column,
            native_number: target.native_number,
        });
        prepared.bind(request_id);
        self.generation = request_id;
        self.pending = PendingOperation::FieldRenameInspection { request_id };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn publish_field_rename_inspection(
        &mut self,
        generation: u64,
        inspection: FieldRenameSourceInspection,
    ) {
        if !matches!(self.pending, PendingOperation::FieldRenameInspection { request_id } if generation == request_id)
        {
            return;
        }
        self.complete_recorded_field_rename_inspection(generation, inspection);
    }

    fn complete_recorded_field_rename_inspection(
        &mut self,
        generation: u64,
        inspection: FieldRenameSourceInspection,
    ) {
        if !self.state.set_field_rename_inspection_by_id(
            generation,
            inspection.document_revision,
            &inspection.table,
            &inspection.column,
            inspection.native_number,
            inspection.source_eligible,
            inspection.same_width_ascii_utf16_units,
        ) {
            self.state.fail_field_rename_inspection_by_id(
                generation,
                self.state.localizer.text("objects-open-refused"),
            );
        }
    }

    pub(in crate::workspace) fn field_rename_inspection_failed(
        &mut self,
        generation: u64,
        message: String,
    ) {
        self.state
            .fail_field_rename_inspection_by_id(generation, message);
    }

    /// Revoked publication means no terminal reply can arrive. Retire only the
    /// exact recorded readonly slot; never clear another operation's busy state.
    pub(in crate::workspace) fn retire_field_rename_inspection(&mut self) {
        let Some(request_id) = self.state.pending_field_rename_inspection_id() else {
            return;
        };
        if !self.worker.publication_is_current(request_id) {
            self.state.interrupt_field_rename_inspection_id(request_id);
        }
    }

    pub(in crate::workspace) fn finish_superseded_field_rename_inspection(
        &mut self,
        generation: u64,
        result: Result<Response, Error>,
    ) {
        if self.state.pending_field_rename_inspection_id() != Some(generation) {
            return;
        }
        if !self.worker.publication_is_current(generation) {
            self.state.interrupt_field_rename_inspection_id(generation);
            return;
        }
        match result {
            Ok(Response::FieldRenameSourceInspected { inspection }) => {
                self.complete_recorded_field_rename_inspection(generation, inspection);
            }
            Err(error) => {
                self.state
                    .fail_field_rename_inspection_by_id(generation, error.to_string());
            }
            Ok(_) => {
                self.state.fail_field_rename_inspection_by_id(
                    generation,
                    self.state.localizer.text("objects-open-refused"),
                );
            }
        }
    }
}
