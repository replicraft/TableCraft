//! Bridge reviewed row insertion to the shared, nonreplaceable save owner.
use super::{ActiveSave, Desktop, PendingOperation, SaveOwner};
use tablecraft_runtime::{
    Command, EditInput as RuntimeEditInput, InsertDraft as RuntimeDraft,
    InsertInput as RuntimeInput, InsertReview as RuntimeReview, InsertTarget as RuntimeTarget,
    ReviewedInsert, journal_path,
};
use tablecraft_ui::{EditInput, InsertDraft, InsertField, InsertInput, InsertReview, InsertTarget};

impl Desktop {
    pub(in crate::workspace) fn begin_insert(&mut self, target: InsertTarget) {
        if self.editing.has_save() || !self.state.can_insert_row(&target) {
            return;
        }
        self.editing.clear_review();
        let id = self.worker.request(Command::BeginInsert {
            target: runtime_target(&target),
        });
        self.generation = id;
        self.pending = PendingOperation::Insert { request_id: id };
        self.state.begin_insert(target, id);
        self.state.busy = true;
    }

    pub(in crate::workspace) fn review_insert(&mut self, draft_id: u64, inputs: Vec<InsertInput>) {
        if self.editing.has_save() || !self.state.can_review_insert(draft_id, &inputs) {
            return;
        }
        self.editing.clear_review();
        let id = self.worker.request(Command::ReviewInsert {
            draft_id,
            inputs: inputs.iter().map(runtime_input).collect(),
        });
        self.generation = id;
        self.pending = PendingOperation::Insert { request_id: id };
        self.state.busy = true;
        self.state.begin_insert_review(draft_id, inputs, id);
    }

    pub(in crate::workspace) fn publish_insert_draft(
        &mut self,
        request_id: u64,
        draft: RuntimeDraft,
    ) {
        self.state.set_insert_draft(
            request_id,
            InsertDraft {
                id: draft.id,
                target: ui_target(draft.target),
                fields: draft
                    .fields
                    .into_iter()
                    .map(|field| InsertField {
                        name: field.name,
                        input_hint: crate::view_models::input_hint(field.input_hint),
                        nullable: field.nullable,
                        auto_number: field.auto_number,
                    })
                    .collect(),
            },
        );
    }

    pub(in crate::workspace) fn publish_insert_review(
        &mut self,
        request_id: u64,
        review: RuntimeReview,
        capability: ReviewedInsert,
    ) {
        let id = review.id;
        if self.state.set_insert_review(
            request_id,
            InsertReview {
                id,
                draft_id: review.draft_id,
                target: ui_target(review.target),
                inputs: review.inputs.into_iter().map(ui_input).collect(),
                inserted_rows: review.inserted_rows,
            },
        ) {
            self.editing.insert_review = Some((id, capability));
        }
    }

    pub(in crate::workspace) fn close_insert(&mut self, request_id: u64) {
        if self.editing.has_save() {
            return;
        }
        if matches!(self.pending, PendingOperation::Insert { request_id: active } if active == request_id)
            && self.generation == request_id
        {
            self.editing.clear_review();
            self.worker.cancel();
            self.state.busy = false;
            self.pending = PendingOperation::Read;
            self.state.close_insert();
        }
    }

    pub(in crate::workspace) fn apply_insert(&mut self, review_id: u64) {
        if self.editing.has_save() || !self.state.can_apply_insert(review_id) {
            return;
        }
        let Some((id, capability)) = self.editing.insert_review.take() else {
            return;
        };
        let source_matches = self.state.database.as_ref().is_some_and(|database| {
            database.document_revision == capability.target().document_revision
        }) && self.state.data.as_ref().is_some_and(|data| {
            data.source_revision == Some(capability.target().document_revision)
                && data.table == capability.target().table
        });
        if id != review_id || !source_matches {
            self.editing.insert_review = Some((id, capability));
            return;
        }
        let path = capability.source_path().to_path_buf();
        let table = capability.target().table.clone();
        match self
            .editing
            .worker
            .start_insert(capability, journal_path(&path))
        {
            Ok(save_id) => {
                self.editing.save = Some(ActiveSave {
                    id: save_id,
                    path,
                    table,
                    offset: 0,
                    report: None,
                    owner: SaveOwner::Insert,
                });
                self.state.begin_insert_save(review_id, save_id);
                self.state.busy = false;
            }
            Err(error) => {
                let message = error.to_string();
                self.editing.insert_review =
                    error.into_reviewed().map(|capability| (id, capability));
                self.state.fail_insert_apply(review_id, message);
            }
        }
    }
}

fn runtime_target(target: &InsertTarget) -> RuntimeTarget {
    RuntimeTarget {
        document_revision: target.document_revision,
        table: target.table.clone(),
    }
}

fn ui_target(target: RuntimeTarget) -> InsertTarget {
    InsertTarget {
        document_revision: target.document_revision,
        table: target.table,
    }
}

fn runtime_input(input: &InsertInput) -> RuntimeInput {
    match input {
        InsertInput::Omitted => RuntimeInput::Omitted,
        InsertInput::Value(value) => RuntimeInput::Value(RuntimeEditInput {
            text: value.text.clone(),
            is_null: value.is_null,
        }),
    }
}

fn ui_input(input: RuntimeInput) -> InsertInput {
    match input {
        RuntimeInput::Omitted => InsertInput::Omitted,
        RuntimeInput::Value(value) => InsertInput::Value(EditInput {
            text: value.text,
            is_null: value.is_null,
        }),
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests;
