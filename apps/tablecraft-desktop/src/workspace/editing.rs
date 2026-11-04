//! Native host dispatch only; typed admission, parsing and persistence stay runtime-owned.
use super::{Desktop, PendingOperation};
use crate::view_models::cell;
use std::path::PathBuf;
use tablecraft_runtime::{
    Command, EditDraft as RuntimeDraft, EditInput as RuntimeInput, EditReview as RuntimeReview,
    EditTarget as RuntimeTarget, ReviewedEdit, SaveCompletion, SaveFailure, SaveFailureKind,
    SaveKnownOutcome, SaveOutcome, SaveReceipt, SaveReport, SaveWorker, journal_path,
};
use tablecraft_ui::{EditDraft, EditInput, EditReview, EditSaveOutcome, EditTarget};

pub(super) struct Editing {
    worker: SaveWorker,
    review: Option<(u64, ReviewedEdit)>,
    save: Option<ActiveSave>,
    refresh: Option<(u64, String, u32)>,
}

struct ActiveSave {
    id: u64,
    path: PathBuf,
    table: String,
    offset: u32,
    report: Option<SaveReport>,
}

impl Editing {
    pub(super) fn new(ctx: egui::Context) -> Self {
        Self {
            worker: SaveWorker::new(move || ctx.request_repaint()),
            review: None,
            save: None,
            refresh: None,
        }
    }

    pub(super) fn has_save(&self) -> bool {
        self.save.is_some()
    }

    pub(super) fn clear_refresh(&mut self) {
        self.refresh = None;
    }

    pub(super) fn clear_review(&mut self) {
        self.review = None;
    }
}

impl Desktop {
    pub(super) fn begin_edit(&mut self, target: EditTarget) {
        if self.editing.has_save() || !self.state.can_edit_cell(&target) {
            return;
        }
        self.editing.clear_review();
        let id = self.worker.request(Command::BeginEdit {
            target: RuntimeTarget {
                document_revision: target.document_revision,
                table: target.table.clone(),
                column: target.column.clone(),
                parent_ordinal: target.parent_ordinal,
            },
        });
        self.generation = id;
        self.pending = PendingOperation::Edit { request_id: id };
        self.state.busy = true;
        self.state.begin_edit(target, id);
    }

    pub(super) fn review_edit(&mut self, draft_id: u64, input: EditInput) {
        if self.editing.has_save() || !self.state.can_review_edit(draft_id, &input) {
            return;
        }
        self.editing.clear_review();
        let id = self.worker.request(Command::ReviewEdit {
            draft_id,
            input: RuntimeInput {
                text: input.text.clone(),
                is_null: input.is_null,
            },
        });
        self.generation = id;
        self.pending = PendingOperation::Edit { request_id: id };
        self.state.busy = true;
        self.state.begin_edit_review(draft_id, input, id);
    }

    pub(super) fn publish_edit_draft(&mut self, request_id: u64, draft: RuntimeDraft) {
        self.state.set_edit_draft(
            request_id,
            EditDraft {
                id: draft.id,
                target: ui_target(draft.target),
                original: cell(draft.original),
                initial: EditInput {
                    text: draft.initial.text,
                    is_null: draft.initial.is_null,
                },
                input_kind: draft.input_kind,
                nullable: draft.nullable,
            },
        );
    }

    pub(super) fn publish_edit_review(
        &mut self,
        request_id: u64,
        review: RuntimeReview,
        capability: ReviewedEdit,
    ) {
        let id = review.id;
        if self.state.set_edit_review(
            request_id,
            EditReview {
                id,
                draft_id: review.draft_id,
                target: ui_target(review.target),
                before: cell(review.before),
                after: cell(review.after),
                changed_rows: review.changed_rows,
            },
        ) {
            self.editing.review = Some((id, capability));
        }
    }

    pub(super) fn close_edit(&mut self, request_id: u64) {
        if self.editing.has_save() {
            return;
        }
        if matches!(self.pending, PendingOperation::Edit { request_id: active } if active == request_id)
            && self.generation == request_id
        {
            self.editing.clear_review();
            self.worker.cancel();
            self.state.busy = false;
            self.pending = PendingOperation::Read;
            self.state.close_edit();
        }
    }

    pub(super) fn apply_edit(&mut self, review_id: u64) {
        if self.editing.has_save() || !self.state.can_apply_edit(review_id) {
            return;
        }
        let Some((id, reviewed)) = self.editing.review.take() else {
            return;
        };
        if id != review_id
            || !self
                .state
                .can_edit_cell(&ui_target(reviewed.target().clone()))
        {
            self.editing.review = Some((id, reviewed));
            return;
        }
        let path = reviewed.source_path().to_path_buf();
        let table = reviewed.target().table.clone();
        let offset = self.state.data.as_ref().map_or(0, |data| data.offset);
        match self.editing.worker.start(reviewed, journal_path(&path)) {
            Ok(save_id) => {
                self.editing.save = Some(ActiveSave {
                    id: save_id,
                    path,
                    table,
                    offset,
                    report: None,
                });
                self.state.begin_edit_save(review_id, save_id);
                self.state.busy = false;
            }
            Err(error) => {
                let message = error.to_string();
                self.editing.review = error.into_reviewed().map(|reviewed| (id, reviewed));
                self.state.fail_edit_apply(review_id, message);
            }
        }
    }

    pub(super) fn poll_saves(&mut self) {
        let Some(report) = self.editing.worker.poll() else {
            return;
        };
        let Some(save) = &mut self.editing.save else {
            return;
        };
        if report.id != save.id || save.report == Some(report) {
            return;
        }
        save.report = Some(report);
        let outcome = save_outcome(report.outcome);
        self.state.set_edit_save_outcome(report.id, outcome);
    }

    pub(super) fn cancel_current_save(&mut self) {
        if let Some(save) = &self.editing.save {
            self.editing.worker.cancel(save.id);
        }
    }

    pub(super) fn cancel_save(&mut self, id: u64) {
        if self.editing.save.as_ref().is_some_and(|save| save.id == id) {
            self.editing.worker.cancel(id);
        }
    }

    pub(super) fn resolve_save(&mut self, id: u64, recover: bool) {
        let Some(save) = &mut self.editing.save else {
            return;
        };
        if save.id != id {
            return;
        }
        let result = if recover {
            self.editing.worker.recover(id)
        } else {
            self.editing.worker.release(id)
        };
        match result {
            Ok(next) => {
                save.id = next;
                save.report = None;
                self.state.begin_edit_resolution(id, next);
            }
            Err(error) => self.state.error = Some(error.to_string()),
        }
    }

    pub(super) fn acknowledge_save(&mut self, id: u64) {
        if !self.editing.save.as_ref().is_some_and(|save| save.id == id)
            || !self.editing.worker.acknowledge(id)
        {
            return;
        }
        let save = self.editing.save.take().unwrap();
        self.state.close_edit();
        if matches!(
            save.report.map(|report| report.outcome),
            Some(SaveOutcome::Saved { .. })
        ) {
            self.select_file(save.path);
            self.open();
            self.editing.refresh = Some((self.generation, save.table, save.offset));
        } else {
            self.state.database = None;
            self.state.data = None;
            self.state.query_data = None;
            self.state.clear_schema();
            self.state.show_open();
        }
    }

    pub(super) fn refresh_after_save(&mut self, generation: u64) {
        if let Some((expected, table, offset)) = self.editing.refresh.take()
            && generation == expected
        {
            self.submit(Command::Page { table, offset });
        }
    }

    pub(super) fn guard_save_close(&mut self, ctx: &egui::Context) {
        if self.editing.has_save() && ctx.input(|input| input.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.cancel_current_save();
        }
    }
}

fn ui_target(target: RuntimeTarget) -> EditTarget {
    EditTarget {
        document_revision: target.document_revision,
        table: target.table,
        column: target.column,
        parent_ordinal: target.parent_ordinal,
    }
}

fn known_result(outcome: Option<SaveKnownOutcome>) -> &'static str {
    match outcome {
        Some(SaveKnownOutcome::Committed) => "committed",
        Some(SaveKnownOutcome::RolledBack) => "rolled back",
        Some(SaveKnownOutcome::OriginalVerified) => "original image verified",
        Some(SaveKnownOutcome::ReplacementVerified) => "replacement image verified",
        None => "not established",
    }
}

fn save_outcome(outcome: SaveOutcome) -> EditSaveOutcome {
    match outcome {
        SaveOutcome::Saved { receipt } => {
            let mut message = String::from("Save completed and lock release was acknowledged.");
            if let Some(changed) = receipt.changed_rows {
                message.push_str(&format!(" Changed rows: {changed}."));
            }
            EditSaveOutcome::Saved(message)
        }
        SaveOutcome::Released {
            current_outcome,
            last_completed,
        } => EditSaveOutcome::Failed(format!(
            "Resources released. Recovery files may remain; resolve them before using this file. {}",
            outcome_details(current_outcome, last_completed),
        )),
        SaveOutcome::Failed(failure) => failure_outcome(failure),
    }
}

fn outcome_details(
    current_outcome: Option<SaveKnownOutcome>,
    last_completed: Option<SaveReceipt>,
) -> String {
    let mut message = format!(
        "Current operation outcome: {}.",
        known_result(current_outcome),
    );
    let Some(receipt) = last_completed else {
        message.push_str(" No completed-operation receipt is available.");
        return message;
    };
    let completion = match receipt.completion {
        SaveCompletion::Applied => "applied",
        SaveCompletion::Recovered => "recovered",
        SaveCompletion::Confirmed => "confirmed",
    };
    message.push_str(&format!(
        " Last completed operation: {completion}; outcome: {}.",
        known_result(Some(receipt.known)),
    ));
    if let Some(matched) = receipt.matched_rows {
        message.push_str(&format!(" Matched rows: {matched}."));
    }
    if let Some(changed) = receipt.changed_rows {
        message.push_str(&format!(" Changed rows: {changed}."));
    }
    message
}

fn failure_reason(kind: SaveFailureKind) -> &'static str {
    match kind {
        SaveFailureKind::StaleDocument => "the viewed document was replaced",
        SaveFailureKind::Open => "the file could not be opened for exclusive editing",
        SaveFailureKind::SourceChanged => "the file changed after it was viewed",
        SaveFailureKind::Cancelled => "cancellation was requested",
        SaveFailureKind::Resource => "an operation limit was reached",
        SaveFailureKind::Planning => "the edit could not be validated",
        SaveFailureKind::Intent => "the change record could not be prepared",
        SaveFailureKind::Setup => "the recovery files could not be set up",
        SaveFailureKind::Prepare => "the save could not be prepared",
        SaveFailureKind::Commit => "the commit did not finish normally",
        SaveFailureKind::Recovery => "recovery did not finish normally",
        SaveFailureKind::Finalization => "save finalization did not finish normally",
        SaveFailureKind::Release => "lock release was not acknowledged",
        SaveFailureKind::InvalidState => "the save session requires diagnosis",
    }
}

fn failure_outcome(failure: SaveFailure) -> EditSaveOutcome {
    let message = format!(
        "Save did not finish normally ({}). {} Do not retry automatically.",
        failure_reason(failure.kind),
        outcome_details(failure.current_outcome, failure.last_completed),
    );
    if failure.state.is_some() {
        EditSaveOutcome::Retained {
            message,
            can_recover: failure.can_recover,
            can_release: failure.can_release,
        }
    } else {
        EditSaveOutcome::Failed(message)
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests;
