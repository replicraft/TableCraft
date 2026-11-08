//! Scratch prompts retain runtime-issued SQL ownership; the host only routes DTOs.
use super::{Desktop, PendingOperation};
use std::sync::Arc;
use tablecraft_runtime::{
    Command, InspectedQuery, ParameterInput as RuntimeInput, ParameterSpec as RuntimeSpec,
    QueryOperation,
};
use tablecraft_ui::{Action, ParameterInput, ParameterSpec};

pub(super) struct Inspected {
    inspection_id: u64,
    target: Arc<InspectedQuery>,
}

impl Desktop {
    pub(super) fn clear_query_parameters(&mut self) {
        self.query_parameters = None;
        self.state.clear_query_parameter_acquisition();
    }

    pub(super) fn release_stale_query_parameters(&mut self) {
        self.release_stale_query_action_review();
        if self.query_parameters.as_ref().is_some_and(|inspected| {
            !self
                .state
                .query_parameter_inspection_current(inspected.inspection_id, inspected.target.sql())
        }) {
            self.query_parameters = None;
        }

        if let PendingOperation::QueryInspection { request_id } = self.pending
            && request_id == self.generation
            && !self
                .state
                .query_parameter_inspection_current(request_id, &self.state.sql)
        {
            self.worker.cancel();
            self.state.busy = false;
            self.clear_query_parameters();
            self.pending = PendingOperation::Read;
        }
    }

    pub(super) fn inspect_query_parameters(&mut self) {
        if self.editing.has_save()
            || self.creating.is_busy()
            || self.recovery.is_some()
            || !self.state.can_inspect_query_parameters()
        {
            return;
        }

        let Some(document_revision) = self
            .state
            .database
            .as_ref()
            .map(|database| database.document_revision)
        else {
            return;
        };

        let Some(sql) = self.state.capture_query_draft() else {
            return;
        };

        self.clear_query_parameters();
        let request_id = self.worker.request(Command::InspectQuery {
            document_revision,
            sql: sql.clone(),
        });
        self.generation = request_id;
        if !self.state.begin_query_parameter_inspection(request_id, sql) {
            self.worker.cancel();
            self.clear_query_parameters();
            self.pending = PendingOperation::Read;
            self.state.busy = false;
            return;
        }

        self.pending = PendingOperation::QueryInspection { request_id };
        self.editing.clear_review();
        self.state.busy = true;
        self.state.error = None;
    }

    pub(super) fn publish_query_parameters(
        &mut self,
        request_id: u64,
        target: Arc<InspectedQuery>,
        parameters: Vec<RuntimeSpec>,
    ) {
        if !matches!(self.pending, PendingOperation::QueryInspection { request_id: actual } if actual == request_id)
        {
            return;
        }
        if !self
            .state
            .query_parameter_inspection_current(request_id, target.sql())
            || self
                .state
                .database
                .as_ref()
                .is_none_or(|database| database.document_revision != target.document_revision())
        {
            self.clear_query_parameters();
            self.pending = PendingOperation::Read;
            return;
        }

        match target.operation() {
            QueryOperation::Unsupported => {
                self.state.fail_query_parameter_inspection(request_id);
                self.state.error = Some(self.state.localizer.text("ribbon-unavailable"));
                self.query_parameters = None;
                self.pending = PendingOperation::Read;
                return;
            }
            QueryOperation::Read | QueryOperation::Update | QueryOperation::Delete => {}
        }

        let specs = parameters
            .into_iter()
            .map(|parameter| ParameterSpec {
                name: parameter.name,
                kind: parameter.kind,
                label: parameter.label,
                nullable: parameter.nullable,
            })
            .collect();

        self.query_parameters = Some(Inspected {
            inspection_id: request_id,
            target,
        });
        let inspected = self
            .query_parameters
            .as_ref()
            .expect("retained runtime inspection");
        let action =
            self.state
                .set_query_parameter_details(request_id, inspected.target.sql(), specs);
        self.pending = PendingOperation::Read;

        if let Some(Action::RunParameterizedQuery {
            inspection_id,
            parameters,
        }) = action
        {
            self.run_parameterized_query(inspection_id, parameters);
        } else {
            self.release_stale_query_parameters();
        }
    }

    pub(super) fn run_parameterized_query(
        &mut self,
        inspection_id: u64,
        parameters: Vec<ParameterInput>,
    ) {
        if self.editing.has_save() || self.creating.is_busy() || self.recovery.is_some() {
            return;
        }

        let Some(inspected) = self
            .query_parameters
            .as_ref()
            .filter(|inspected| inspected.inspection_id == inspection_id)
        else {
            return;
        };

        if !self.state.can_run_parameterized_query(
            inspection_id,
            inspected.target.sql(),
            &parameters,
        ) {
            self.release_stale_query_parameters();
            return;
        }

        let target = Arc::clone(&inspected.target);
        if matches!(
            target.operation(),
            QueryOperation::Update | QueryOperation::Delete
        ) {
            self.begin_query_action_from_parameters(inspection_id, target, parameters);
            return;
        }
        if target.operation() != QueryOperation::Read {
            return;
        }

        let Some(sql) = self.state.capture_query_sql(target.sql()) else {
            return;
        };

        let inputs = runtime_inputs(&parameters);

        let request_id = self.worker.request(Command::RunQuery {
            target,
            parameters: inputs,
        });
        self.generation = request_id;
        if !self
            .state
            .begin_parameterized_query_run(inspection_id, request_id, sql, parameters)
        {
            self.worker.cancel();
            self.clear_query_parameters();
            self.state.clear_query_run();
            self.pending = PendingOperation::Read;
            self.state.busy = false;
            return;
        }

        self.query_parameters = None;
        self.pending = PendingOperation::Query { request_id };
        self.state.busy = true;
        self.state.error = None;
    }
}

pub(in crate::workspace) fn runtime_inputs(parameters: &[ParameterInput]) -> Vec<RuntimeInput> {
    parameters
        .iter()
        .map(|input| RuntimeInput {
            name: input.name.clone(),
            kind: input.kind.clone(),
            text: input.text.clone(),
            is_null: input.is_null,
        })
        .collect()
}

#[cfg(test)]
mod tests;
