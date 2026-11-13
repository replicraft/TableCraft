//! Scratch prompts retain runtime-issued SQL ownership; the host only routes DTOs.
use super::{Desktop, PendingOperation};
use crate::workspace::request_read;
use std::sync::Arc;
use tablecraft_runtime::{
    Command, DiscoveredQuery, InspectedQuery, ParameterInput as RuntimeInput,
    ParameterSpec as RuntimeSpec, QueryOperation, QueryRunOptions,
};
use tablecraft_ui::{Action, ParameterInput, ParameterSpec};

pub(super) struct Inspected {
    inspection_id: u64,
    target: Arc<InspectedQuery>,
    options: QueryRunOptions,
    discovered: Option<Arc<DiscoveredQuery>>,
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

        if let PendingOperation::QueryDiscovery {
            request_id,
            inspection_id,
            ..
        } = self.pending
            && request_id == self.generation
            && self.query_parameters.as_ref().is_none_or(|inspected| {
                !self
                    .state
                    .query_parameter_inspection_current(inspection_id, inspected.target.sql())
            })
        {
            self.worker.cancel();
            self.state.busy = false;
            self.clear_query_parameters();
            self.pending = PendingOperation::Read;
        }

        if let PendingOperation::QueryInspection { request_id, .. } = self.pending
            && request_id == self.generation
            && !self
                .state
                .query_parameter_inspection_owner_current(request_id)
        {
            self.worker.cancel();
            self.state.busy = false;
            self.clear_query_parameters();
            self.pending = PendingOperation::Read;
        }
    }

    pub(super) fn inspect_query_parameters(&mut self) {
        self.inspect_query_parameters_with_options(self.read_query_options());
    }

    /// Both SQL and completed Design runs retain the options captured by their
    /// own originating request while sharing the same typed acquisition service.
    pub(super) fn inspect_query_parameters_with_options(&mut self, options: QueryRunOptions) {
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
        let Some(request_id) = request_read(
            Command::InspectQuery {
                document_revision,
                // NO_CLONE: Worker inspection owns SQL while UI pending acquisition retains the
                // exact captured draft to reject stale SQL/revision responses.
                sql: sql.clone(),
            },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            return;
        };
        self.generation = request_id;
        if !self.state.begin_query_parameter_inspection(request_id, sql) {
            self.worker.cancel();
            self.clear_query_parameters();
            self.pending = PendingOperation::Read;
            self.state.busy = false;
            return;
        }

        self.pending = PendingOperation::QueryInspection {
            request_id,
            options,
        };
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
        let options = match self.pending {
            PendingOperation::QueryInspection {
                request_id: actual,
                options,
            } if actual == request_id => options,
            _ => return,
        };
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
            QueryOperation::Read
            | QueryOperation::Update
            | QueryOperation::Delete
            | QueryOperation::Insert
            | QueryOperation::Schema => {}
        }

        if target.operation() == QueryOperation::Read {
            // NO_CLONE: Discovery and the pending UI witness share the original
            // issued SQL owner across the next metadata-only worker request.
            let original = Arc::clone(&target);
            let Some(next) = request_read(
                Command::DiscoverQueryParameters {
                    options,
                    target: original,
                },
                &self.worker,
                &mut self.state,
            ) else {
                self.state.fail_query_parameter_inspection(request_id);
                self.read_request_rejected();
                return;
            };
            self.query_parameters = Some(Inspected {
                inspection_id: request_id,
                target,
                options,
                discovered: None,
            });
            self.generation = next;
            self.pending = PendingOperation::QueryDiscovery {
                request_id: next,
                inspection_id: request_id,
                options,
            };
            self.state.busy = true;
            return;
        }
        self.publish_query_parameter_prompt(request_id, target, parameters, options, None);
    }

    pub(super) fn publish_discovered_query_parameters(
        &mut self,
        request_id: u64,
        discovered: Arc<DiscoveredQuery>,
        parameters: Vec<RuntimeSpec>,
    ) {
        let (inspection_id, options) = match self.pending {
            PendingOperation::QueryDiscovery {
                request_id: actual,
                inspection_id,
                options,
            } if actual == request_id => (inspection_id, options),
            _ => return,
        };
        let original = discovered.original();
        if !self
            .state
            .query_parameter_inspection_current(inspection_id, original.sql())
            || self
                .state
                .database
                .as_ref()
                .is_none_or(|database| database.document_revision != original.document_revision())
        {
            self.clear_query_parameters();
            self.pending = PendingOperation::Read;
            return;
        }
        // NO_CLONE: The UI keeps the original issued syntax witness while the
        // discovery owner retains the same witness beside its immutable plan.
        let target = Arc::clone(original);
        self.publish_query_parameter_prompt(
            inspection_id,
            target,
            parameters,
            options,
            Some(discovered),
        );
    }

    fn publish_query_parameter_prompt(
        &mut self,
        request_id: u64,
        target: Arc<InspectedQuery>,
        parameters: Vec<RuntimeSpec>,
        options: QueryRunOptions,
        discovered: Option<Arc<DiscoveredQuery>>,
    ) {
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
            options,
            discovered,
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

        let options = inspected.options;
        // NO_CLONE: The inspected target is shared between the immutable runtime binding and
        // dispatched run/action command; sharing avoids copying its parsed binding.
        let target = Arc::clone(&inspected.target);
        if matches!(
            target.operation(),
            QueryOperation::Update
                | QueryOperation::Delete
                | QueryOperation::Insert
                | QueryOperation::Schema
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

        let command = if let Some(discovered) = &inspected.discovered {
            // NO_CLONE: Runtime execution and UI pending provenance share the
            // issued immutable prepared owner; its plan/payload is not copied.
            Command::RunDiscoveredQuery {
                options,
                target: Arc::clone(discovered),
                parameters: inputs,
            }
        } else {
            Command::RunQuery {
                options,
                target,
                parameters: inputs,
            }
        };
        let Some(request_id) = request_read(command, &self.worker, &mut self.state) else {
            self.read_request_rejected();
            return;
        };
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
            // NO_CLONE: Runtime parameter name DTO and shared retained prompt/run snapshot have
            // separate asynchronous owners; input names participate in exact parameter
            // identity.
            name: input.name.clone(),
            // NO_CLONE: Runtime parameter kind DTO and shared retained prompt/run snapshot both
            // retain explicit type choices for parsing and stale-reply validation.
            kind: input.kind.clone(),
            // NO_CLONE: Runtime consumes parameter input text while UI retains original submitted
            // values and previous-result provenance across cancellation/failure.
            text: input.text.clone(),
            is_null: input.is_null,
        })
        .collect()
}

#[cfg(test)]
mod tests;
