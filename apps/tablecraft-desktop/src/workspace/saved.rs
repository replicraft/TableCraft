//! Saved-query dispatch and owned DTO adaptation only.
mod sql;

use super::{Desktop, PendingOperation};
use crate::workspace::request_read;
use tablecraft_runtime::{
    Command, ParameterInput as RuntimeInput, ParameterSpec as RuntimeSpec,
    SavedQueryTarget as RuntimeTarget,
};
use tablecraft_ui::{ObjectKind, ObjectTarget, ParameterInput, ParameterSpec, SavedQueryTarget};

impl Desktop {
    pub(super) fn inspect_saved_query(&mut self, target: SavedQueryTarget) {
        let active = self.state.active_object().is_some_and(|tab| {
            tab.target.document_revision == target.document_revision
                && matches!(&tab.target.kind, ObjectKind::SavedQuery { object_id, name }
                    if *object_id == target.object_id && *name == target.name)
        });
        if !active {
            self.open_object(ObjectTarget {
                document_revision: target.document_revision,
                kind: ObjectKind::SavedQuery {
                    object_id: target.object_id,
                    name: target.name,
                },
            });
            return;
        }
        if self.editing.has_save() || !self.state.can_inspect_saved_query(&target) {
            return;
        }

        let Some(request_id) = request_read(
            Command::InspectSavedQuery {
                target: runtime_target(&target),
            },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            return;
        };
        self.generation = request_id;
        self.pending = PendingOperation::SavedInspection {
            // NO_CLONE: Inspection pending state lives independently in desktop and shared UI; the
            // shared inspection owner stores the actual target for saved SQL and run
            // routing.
            target: target.clone(),
            request_id,
        };
        self.editing.clear_review();
        self.state.begin_saved_query_inspection(target, request_id);
        self.state.busy = true;
        self.state.error = None;
    }

    pub(super) fn run_saved_query(
        &mut self,
        target: SavedQueryTarget,
        parameters: Vec<ParameterInput>,
    ) {
        if self.editing.has_save()
            || !self.state.can_run_saved_query(&target, &parameters)
            || !self.state.can_retain_saved_query_parameters(&parameters)
        {
            return;
        }

        let options = self.read_query_options();
        let inputs = parameters
            .iter()
            .map(|input| RuntimeInput {
                // NO_CLONE: Runtime parameter DTO and retained shared saved-query prompt input each
                // own their name through asynchronous execution.
                name: input.name.clone(),
                // NO_CLONE: Runtime parameter parsing and retained shared saved-query prompt input
                // each own the explicit kind string through run completion.
                kind: input.kind.clone(),
                // NO_CLONE: Runtime consumes submitted parameter text while shared saved-query
                // state retains original values for request/provenance checks and
                // cancellation.
                text: input.text.clone(),
                is_null: input.is_null,
            })
            .collect();
        let Some(request_id) = request_read(
            Command::RunSavedQuery {
                options,
                target: runtime_target(&target),
                parameters: inputs,
            },
            &self.worker,
            &mut self.state,
        ) else {
            self.read_request_rejected();
            return;
        };
        self.generation = request_id;
        self.state
            .begin_saved_query_run(&target, parameters, request_id);
        self.pending = PendingOperation::SavedRun { target, request_id };
        self.state.busy = true;
        self.state.error = None;
    }

    pub(super) fn publish_saved_query(
        &mut self,
        request_id: u64,
        target: RuntimeTarget,
        parameters: Vec<RuntimeSpec>,
    ) {
        let target = SavedQueryTarget {
            document_revision: target.document_revision,
            object_id: target.object_id,
            name: target.name,
        };
        if self.state.set_saved_query_details(
            request_id,
            &target,
            parameters
                .into_iter()
                .map(|parameter| ParameterSpec {
                    name: parameter.name,
                    kind: parameter.kind,
                    label: parameter.label,
                    nullable: parameter.nullable,
                })
                .collect(),
        ) && self.state.saved_query_sql().is_none()
        {
            // A same-revision retained SQL draft already has its source binding.
            // In particular, witnessed own-save publication retains submitted
            // spelling rather than replacing it with canonical inspection SQL.
            self.inspect_saved_query_sql(target);
        }
    }
}

fn runtime_target(target: &SavedQueryTarget) -> RuntimeTarget {
    RuntimeTarget {
        document_revision: target.document_revision,
        object_id: target.object_id,
        // NO_CLONE: The runtime command target and shared saved-query state require independent
        // names; runtime_target converts a borrowed retained UI target.
        name: target.name.clone(),
    }
}

#[cfg(test)]
mod tests;
