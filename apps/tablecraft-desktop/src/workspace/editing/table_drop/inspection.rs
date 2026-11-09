//! Exact readonly native facts enable confirmation without becoming write authority.
use super::TableDropObservation;
use crate::workspace::{Desktop, PendingOperation, request_read};
use std::sync::Arc;
use tablecraft_runtime::{Command, Error, Response, TableDropSourceInspection};
use tablecraft_ui::TableDropRequest;

impl Desktop {
    pub(in crate::workspace) fn inspect_table_drop_source(
        &mut self,
        request: Arc<TableDropRequest>,
    ) {
        if self.editing.has_save() || self.state.table_drop_request() != Some(request.as_ref()) {
            self.state.reject_table_drop_inspection(
                &request,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        }
        let Some(table) = request.table_name() else {
            return;
        };
        // NO_CLONE: The bounded runtime command owns its selected name on another thread;
        // the immutable Arc request stays with UI/host for exact inspection and review checks.
        let command = Command::InspectTableDropSource {
            document_revision: request.target.document_revision,
            table: table.to_owned(),
        };
        let Some(request_id) = request_read(command, &self.worker, &mut self.state) else {
            self.state.reject_table_drop_inspection(
                &request,
                self.state
                    .localizer
                    .text("objects-read-identities-exhausted"),
            );
            self.read_request_rejected();
            return;
        };
        if !self.state.begin_table_drop_inspection(&request, request_id) {
            self.worker.cancel();
            return;
        }
        self.editing.table_drop_observation = Some(TableDropObservation {
            request_id,
            request,
            facts: None,
        });
        self.generation = request_id;
        self.pending = PendingOperation::TableDropInspection { request_id };
        self.state.busy = true;
    }

    pub(in crate::workspace) fn publish_table_drop_inspection(
        &mut self,
        generation: u64,
        inspection: TableDropSourceInspection,
    ) {
        let Some(observation) = self
            .editing
            .table_drop_observation
            .as_mut()
            .filter(|observation| observation.request_id == generation)
        else {
            return;
        };
        let exact = inspection.document_revision == observation.request.target.document_revision
            && observation.request.table_name() == Some(inspection.table.as_str());
        if !exact {
            self.state.fail_table_drop_inspection(
                &observation.request,
                generation,
                self.state.localizer.text("objects-open-refused"),
            );
            return;
        }
        if self.state.set_table_drop_source(
            &observation.request,
            generation,
            inspection
                .source_eligible
                .then_some(inspection.declared_rows),
        ) {
            observation.facts = inspection
                .source_eligible
                .then_some((inspection.native_id, inspection.declared_rows));
        }
    }

    pub(in crate::workspace) fn table_drop_inspection_failed(
        &mut self,
        generation: u64,
        message: String,
    ) {
        let Some(observation) = self
            .editing
            .table_drop_observation
            .as_ref()
            .filter(|observation| observation.request_id == generation)
        else {
            return;
        };
        self.state
            .fail_table_drop_inspection(&observation.request, generation, message);
        self.editing.table_drop_observation = None;
    }

    pub(in crate::workspace) fn retire_table_drop_inspection(&mut self) {
        let Some(observation) = &self.editing.table_drop_observation else {
            return;
        };
        if self.state.table_drop_request() != Some(observation.request.as_ref()) {
            self.editing.table_drop_observation = None;
            return;
        }
        if observation.facts.is_none()
            && !self.worker.publication_is_current(observation.request_id)
        {
            self.state
                .interrupt_table_drop_inspection(observation.request_id);
            self.editing.table_drop_observation = None;
        }
    }

    pub(in crate::workspace) fn owns_table_drop_inspection(&self, generation: u64) -> bool {
        self.editing
            .table_drop_observation
            .as_ref()
            .is_some_and(|observation| observation.request_id == generation)
    }

    pub(in crate::workspace) fn finish_superseded_table_drop_inspection(
        &mut self,
        generation: u64,
        result: Result<Response, Error>,
    ) -> bool {
        let Some(observation) = self
            .editing
            .table_drop_observation
            .as_ref()
            .filter(|observation| observation.request_id == generation)
        else {
            return false;
        };
        if !self.worker.publication_is_current(generation) {
            self.state
                .interrupt_table_drop_inspection(observation.request_id);
            self.editing.table_drop_observation = None;
            return true;
        }
        match result {
            Ok(Response::TableDropSourceInspected { inspection }) => {
                self.publish_table_drop_inspection(generation, inspection)
            }
            Err(error) => self.table_drop_inspection_failed(generation, error.to_string()),
            Ok(_) => self.table_drop_inspection_failed(
                generation,
                self.state.localizer.text("objects-open-refused"),
            ),
        }
        true
    }

    pub(in crate::workspace) fn cancel_table_drop_inspection(&mut self, request_id: u64) {
        if self.generation != request_id
            || !matches!(self.pending, PendingOperation::TableDropInspection { request_id: current } if current == request_id)
        {
            return;
        }
        if self.worker.publication_is_current(request_id) {
            self.worker.cancel();
        }
        self.state.interrupt_table_drop_inspection(request_id);
        self.editing.table_drop_observation = None;
        self.pending = PendingOperation::Read;
        self.state.busy = false;
    }
}
