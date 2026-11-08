//! Saved-query dispatch and owned DTO adaptation only.
mod sql;

use super::{Desktop, PendingOperation};
use tablecraft_runtime::{
    Command, ParameterInput as RuntimeInput, ParameterSpec as RuntimeSpec,
    SavedQueryTarget as RuntimeTarget,
};
use tablecraft_ui::{ObjectKind, ObjectTarget, ParameterInput, ParameterSpec, SavedQueryTarget};

impl Desktop {
    pub(super) fn inspect_saved_query(&mut self, target: SavedQueryTarget) {
        let object = ObjectTarget {
            document_revision: target.document_revision,
            kind: ObjectKind::SavedQuery {
                object_id: target.object_id,
                name: target.name.clone(),
            },
        };
        if self.state.active_object().map(|tab| tab.target).as_ref() != Some(&object) {
            self.open_object(object);
            return;
        }
        if self.editing.has_save() || !self.state.can_inspect_saved_query(&target) {
            return;
        }

        let request_id = self.worker.request(Command::InspectSavedQuery {
            target: runtime_target(&target),
        });
        self.generation = request_id;
        self.pending = PendingOperation::SavedInspection {
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

        let inputs = parameters
            .iter()
            .map(|input| RuntimeInput {
                name: input.name.clone(),
                kind: input.kind.clone(),
                text: input.text.clone(),
                is_null: input.is_null,
            })
            .collect();
        let request_id = self.worker.request(Command::RunSavedQuery {
            target: runtime_target(&target),
            parameters: inputs,
        });
        self.generation = request_id;
        self.pending = PendingOperation::SavedRun {
            target: target.clone(),
            request_id,
        };
        self.state
            .begin_saved_query_run(target, parameters, request_id);
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
        let selected = target.clone();
        if self.state.set_saved_query_details(
            request_id,
            target,
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
            self.inspect_saved_query_sql(selected);
        }
    }
}

fn runtime_target(target: &SavedQueryTarget) -> RuntimeTarget {
    RuntimeTarget {
        document_revision: target.document_revision,
        object_id: target.object_id,
        name: target.name.clone(),
    }
}

#[cfg(test)]
mod tests;
