//! Native host dispatch only; typed admission, parsing and persistence stay runtime-owned.
mod deleting;
pub(super) mod first_table;
mod inserting;
#[cfg(test)]
mod pasting;
use super::{Desktop, PendingOperation, objects::Sessions};
use crate::view_models::cell;
use std::path::PathBuf;
use tablecraft_runtime::{
    Command, EditDraft as RuntimeDraft, EditInput as RuntimeInput, EditReview as RuntimeReview,
    EditTarget as RuntimeTarget, ReviewedDelete, ReviewedEdit, ReviewedInsert,
    SaveCleanupDisposition, SaveCompletion, SaveFailure, SaveFailureKind, SaveKnownOutcome,
    SaveOutcome, SaveReceipt, SaveReport, SaveWorker, journal_path,
};
use tablecraft_ui::{
    EditDraft, EditInput, EditReview, EditSaveOutcome, EditTarget, FluentArgs, Localizer,
    SaveCleanupNotice,
};

pub(super) struct Editing {
    worker: SaveWorker,
    review: Option<(u64, ReviewedEdit)>,
    insert_review: Option<(u64, ReviewedInsert)>,
    delete_review: Option<(u64, ReviewedDelete)>,
    save: Option<ActiveSave>,
    refresh: Option<(u64, String, u32)>,
    refresh_cleanup: Option<SaveCleanupNotice>,
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
impl Editing {
    pub(super) fn save_report(&self) -> Option<SaveReport> {
        self.save.as_ref().and_then(|save| save.report)
    }
}

#[derive(Clone, Copy)]
enum SaveOwner {
    Cell,
    Insert,
    Delete,
    FirstTable,
}

struct ActiveSave {
    owner: SaveOwner,
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
            insert_review: None,
            delete_review: None,
            save: None,
            refresh: None,
            refresh_cleanup: None,
        }
    }

    pub(super) fn has_save(&self) -> bool {
        self.save.is_some()
    }

    pub(super) fn clear_refresh(&mut self) {
        self.refresh = None;
        self.refresh_cleanup = None;
    }

    pub(super) fn clear_review(&mut self) {
        self.review = None;
        self.insert_review = None;
        self.delete_review = None;
    }
}

impl Desktop {
    pub(super) fn begin_edit(&mut self, target: EditTarget) {
        if self.editing.has_save() || !self.state.can_edit_cell(&target) {
            return;
        }
        self.request_edit(target, None);
    }

    pub(super) fn begin_paste_edit(&mut self, target: EditTarget, text: String) {
        if self.editing.has_save() || !self.state.can_paste_text(&target, &text) {
            return;
        }
        self.request_edit(target, Some(text));
    }

    fn request_edit(&mut self, target: EditTarget, pasted_text: Option<String>) {
        self.editing.clear_review();
        let id = self.worker.request(Command::BeginEdit {
            target: RuntimeTarget {
                document_revision: target.document_revision,
                table: target.table.clone(),
                column: target.column.clone(),
                parent_ordinal: target.parent_ordinal,
            },
        });
        if let Some(text) = pasted_text {
            if !self.state.begin_paste_edit(target, text, id) {
                self.worker.cancel();
                return;
            }
        } else {
            self.state.begin_edit(target, id);
        }
        self.generation = id;
        self.pending = PendingOperation::Edit { request_id: id };
        self.state.busy = true;
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
        self.state.set_edit_draft(request_id, ui_draft(draft));
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
                    owner: SaveOwner::Cell,
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
        if matches!(save.owner, SaveOwner::FirstTable) {
            self.first_table_save_report(report);
            return;
        }
        let outcome = save_outcome(&self.state.localizer, report.outcome);
        match save.owner {
            SaveOwner::Cell => self.state.set_edit_save_outcome(report.id, outcome),
            SaveOwner::Insert => self.state.set_insert_save_outcome(report.id, outcome),
            SaveOwner::Delete => self.state.set_delete_save_outcome(report.id, outcome),
            SaveOwner::FirstTable => {}
        }
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
                match save.owner {
                    SaveOwner::Cell => self.state.begin_edit_resolution(id, next),
                    SaveOwner::Insert => self.state.begin_insert_resolution(id, next),
                    SaveOwner::Delete => self.state.begin_delete_resolution(id, next),
                    SaveOwner::FirstTable => {}
                }
            }
            Err(error) => self.state.error = Some(error.to_string()),
        }
    }

    pub(super) fn acknowledge_save(&mut self, id: u64) {
        let Some(save) = self.editing.save.as_ref().filter(|save| save.id == id) else {
            return;
        };
        let workspace_restore = if matches!(
            save.report.map(|report| report.outcome),
            Some(SaveOutcome::Saved { .. })
        ) {
            let Some(restore) = self.capture_workspace_restore() else {
                return;
            };
            Some(restore)
        } else {
            None
        };
        if !self.editing.worker.acknowledge(id) {
            return;
        }
        let save = self.editing.save.take().unwrap();
        match save.owner {
            SaveOwner::Cell => {
                self.state.close_edit();
            }
            SaveOwner::Insert => {
                self.state.close_insert();
            }
            SaveOwner::Delete => {
                self.state.close_delete();
            }
            SaveOwner::FirstTable => {}
        }
        if matches!(
            save.report.map(|report| report.outcome),
            Some(SaveOutcome::Saved { .. })
        ) {
            self.select_file(save.path);
            self.open();
            if let Some(restore) = workspace_restore {
                self.retain_workspace_restore(restore);
            }
            self.editing.refresh = Some((self.generation, save.table, save.offset));
            self.editing.refresh_cleanup = match save.report.map(|report| report.outcome) {
                Some(SaveOutcome::Saved { receipt }) => cleanup_notice(receipt),
                _ => None,
            };
        } else {
            self.filter = None;
            self.objects = Sessions::default();
            self.state.database = None;
            self.state.data = None;
            self.state.query_data = None;
            self.state.clear_schema();
            self.state.show_open();
        }
    }

    pub(super) fn refresh_after_save(&mut self, generation: u64) -> bool {
        let cleanup = self.editing.refresh_cleanup.take();
        if let Some((expected, table, offset)) = self.editing.refresh.take()
            && generation == expected
        {
            if let Some(database) = &self.state.database {
                self.state
                    .set_save_cleanup(database.document_revision, cleanup);
            }
            if !self.restore_workspace_after_save(&table) {
                return true;
            }
            self.load_active_object(offset);
            return true;
        }
        false
    }

    pub(super) fn guard_save_close(&mut self, ctx: &egui::Context) {
        if !ctx.input(|input| input.viewport().close_requested()) {
            return;
        }
        if self.editing.has_save() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.cancel_current_save();
        } else if self.state.insertion_dialog_open() {
            // An inline record is still user input, not a persisted native row.
            // Keep it until the user explicitly saves or abandons the record.
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
    }
}

pub(super) fn ui_draft(draft: RuntimeDraft) -> EditDraft {
    EditDraft {
        id: draft.id,
        target: ui_target(draft.target),
        original: cell(draft.original),
        initial: EditInput {
            text: draft.initial.text,
            is_null: draft.initial.is_null,
        },
        input_hint: crate::view_models::input_hint(draft.input_hint),
        nullable: draft.nullable,
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

fn message(localizer: &Localizer, key: &str, values: &[(&str, String)]) -> String {
    let mut arguments = FluentArgs::new();
    for (name, value) in values {
        arguments.set(*name, value.as_str());
    }
    localizer.format(key, &arguments)
}

fn known_result(localizer: &Localizer, outcome: Option<SaveKnownOutcome>) -> String {
    let key = match outcome {
        Some(SaveKnownOutcome::Committed) => "editing-save-known-committed",
        Some(SaveKnownOutcome::RolledBack) => "editing-save-known-rolled-back",
        Some(SaveKnownOutcome::OriginalVerified) => "editing-save-known-original",
        Some(SaveKnownOutcome::ReplacementVerified) => "editing-save-known-replacement",
        None => "editing-save-known-unknown",
    };
    localizer.text(key)
}

fn save_outcome(localizer: &Localizer, outcome: SaveOutcome) -> EditSaveOutcome {
    match outcome {
        SaveOutcome::Saved { receipt } => {
            let mut report = localizer.text("editing-save-completed");
            if let Some(changed) = receipt.changed_rows {
                report.push(' ');
                report.push_str(&message(
                    localizer,
                    "editing-save-changed",
                    &[("count", changed.to_string())],
                ));
            }
            append_cleanup(localizer, receipt, &mut report);
            EditSaveOutcome::Saved(report)
        }
        SaveOutcome::Released {
            current_outcome,
            last_completed,
        } => EditSaveOutcome::Failed(message(
            localizer,
            "editing-save-released",
            &[(
                "details",
                outcome_details(localizer, current_outcome, last_completed),
            )],
        )),
        SaveOutcome::Failed(failure) => failure_outcome(localizer, failure),
    }
}

fn outcome_details(
    localizer: &Localizer,
    current_outcome: Option<SaveKnownOutcome>,
    last_completed: Option<SaveReceipt>,
) -> String {
    let mut report = message(
        localizer,
        "editing-save-current",
        &[("outcome", known_result(localizer, current_outcome))],
    );
    let Some(receipt) = last_completed else {
        report.push(' ');
        report.push_str(&localizer.text("editing-save-no-receipt"));
        return report;
    };
    let completion = localizer.text(match receipt.completion {
        SaveCompletion::Applied => "editing-save-completion-applied",
        SaveCompletion::Recovered => "editing-save-completion-recovered",
        SaveCompletion::Confirmed => "editing-save-completion-confirmed",
    });
    report.push(' ');
    report.push_str(&message(
        localizer,
        "editing-save-prior",
        &[
            ("completion", completion),
            ("outcome", known_result(localizer, Some(receipt.known))),
        ],
    ));
    if let Some(matched) = receipt.matched_rows {
        report.push(' ');
        report.push_str(&message(
            localizer,
            "editing-save-matched",
            &[("count", matched.to_string())],
        ));
    }
    if let Some(changed) = receipt.changed_rows {
        report.push(' ');
        report.push_str(&message(
            localizer,
            "editing-save-changed",
            &[("count", changed.to_string())],
        ));
    }
    append_cleanup(localizer, receipt, &mut report);
    report
}

fn cleanup_notice(receipt: SaveReceipt) -> Option<SaveCleanupNotice> {
    match receipt.cleanup {
        Some(SaveCleanupDisposition::RetainedArtifact) => {
            Some(SaveCleanupNotice::RetainedPrivateJournal)
        }
        Some(SaveCleanupDisposition::Removed) | None => None,
    }
}

fn append_cleanup(localizer: &Localizer, receipt: SaveReceipt, report: &mut String) {
    if cleanup_notice(receipt).is_some() {
        report.push(' ');
        report.push_str(&localizer.text("editing-save-retained-private-journal"));
    }
}

fn failure_reason(localizer: &Localizer, kind: SaveFailureKind) -> String {
    let key = match kind {
        SaveFailureKind::StaleDocument => "editing-save-reason-stale",
        SaveFailureKind::Open => "editing-save-reason-open",
        SaveFailureKind::SourceChanged => "editing-save-reason-source",
        SaveFailureKind::Cancelled => "editing-save-reason-cancelled",
        SaveFailureKind::Resource => "editing-save-reason-resource",
        SaveFailureKind::Planning => "editing-save-reason-planning",
        SaveFailureKind::Intent => "editing-save-reason-intent",
        SaveFailureKind::Setup => "editing-save-reason-setup",
        SaveFailureKind::Prepare => "editing-save-reason-prepare",
        SaveFailureKind::Commit => "editing-save-reason-commit",
        SaveFailureKind::Recovery => "editing-save-reason-recovery",
        SaveFailureKind::Finalization => "editing-save-reason-finalization",
        SaveFailureKind::Release => "editing-save-reason-release",
        SaveFailureKind::InvalidState => "editing-save-reason-state",
    };
    localizer.text(key)
}

fn failure_outcome(localizer: &Localizer, failure: SaveFailure) -> EditSaveOutcome {
    let report = message(
        localizer,
        "editing-save-failure",
        &[
            ("reason", failure_reason(localizer, failure.kind)),
            (
                "details",
                outcome_details(localizer, failure.current_outcome, failure.last_completed),
            ),
        ],
    );
    if failure.state.is_some() {
        EditSaveOutcome::Retained {
            message: report,
            can_recover: failure.can_recover,
            can_release: failure.can_release,
        }
    } else {
        EditSaveOutcome::Failed(report)
    }
}

#[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
mod tests;
