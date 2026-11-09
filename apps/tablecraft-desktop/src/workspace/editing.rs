//! Native host dispatch only; typed admission, parsing and persistence stay runtime-owned.
mod deleting;
mod field_addition;
mod inserting;
#[cfg(test)]
mod pasting;
mod query_action;
mod query_definition;
pub(super) mod recovery;
pub(super) mod table_creation;
pub(super) mod table_rename;
use super::{Desktop, PendingOperation, objects::Sessions};
use crate::view_models::cell;
use inserting::retry::{InsertRetryContext, RetainedInsertRetry};
use std::{path::PathBuf, sync::Arc};
use tablecraft_runtime::{
    Command, EditDraft as RuntimeDraft, EditInput as RuntimeInput, EditReview as RuntimeReview,
    EditTarget as RuntimeTarget, InspectedQuery, ReviewedDelete, ReviewedEdit, ReviewedInsert,
    SaveCleanupDisposition, SaveCompletion, SaveFailure, SaveFailureKind, SaveKnownOutcome,
    SaveOutcome, SaveReceipt, SaveReport, SaveWorker, journal_path,
};
use tablecraft_ui::{
    EditDraft, EditInput, EditReview, EditSaveOutcome, EditTarget, FluentArgs,
    InsertRetryPermission, Localizer, ObjectKind, SaveCleanupNotice,
};

pub(super) struct Editing {
    worker: SaveWorker,
    query_definition: Option<query_definition::QueryDefinitionHost>,
    field_addition: Option<field_addition::FieldAdditionHost>,
    review: Option<(u64, ReviewedEdit)>,
    insert_review: Option<(u64, ReviewedInsert)>,
    delete_review: Option<(u64, ReviewedDelete)>,
    query_action_review: Option<(u64, query_action::ReviewedAction)>,
    query_action_pending: Option<(u64, Arc<InspectedQuery>)>,
    save: Option<ActiveSave>,
    insert_retry: Option<RetainedInsertRetry>,
    refresh: Option<(u64, String, u32)>,
    refresh_cleanup: Option<SaveCleanupNotice>,
    refresh_query_action: bool,
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
    TableCreation,
    TableRename,
    FieldAddition,
    QueryAction,
    QueryDefinition,
}

struct ActiveSave {
    owner: SaveOwner,
    id: u64,
    path: PathBuf,
    table: String,
    offset: u32,
    report: Option<SaveReport>,
    insert_retry: Option<InsertRetryContext>,
}

impl Editing {
    pub(super) fn new(ctx: egui::Context) -> Self {
        Self {
            worker: SaveWorker::new(move || ctx.request_repaint()),
            query_definition: None,
            field_addition: None,
            review: None,
            insert_review: None,
            delete_review: None,
            query_action_review: None,
            query_action_pending: None,
            save: None,
            insert_retry: None,
            refresh: None,
            refresh_cleanup: None,
            refresh_query_action: false,
        }
    }

    pub(super) fn has_save(&self) -> bool {
        self.save.is_some()
            || self.worker.is_busy()
            || self.query_definition.is_some()
            || self.field_addition.is_some()
    }

    pub(super) fn clear_refresh(&mut self) {
        self.refresh = None;
        self.refresh_cleanup = None;
        self.refresh_query_action = false;
    }

    pub(super) fn clear_review(&mut self) {
        self.review = None;
        self.insert_review = None;
        self.delete_review = None;
        self.query_action_review = None;
        self.query_action_pending = None;
    }

    pub(super) fn clear_insert_retry(&mut self) {
        self.insert_retry = None;
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
                    insert_retry: None,
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
        if let Some(retry) = &mut save.insert_retry {
            retry.evidence.observe(report);
        }
        if matches!(save.owner, SaveOwner::QueryDefinition) {
            self.query_definition_save_report(report);
            return;
        }
        if matches!(save.owner, SaveOwner::FieldAddition) {
            self.field_addition_save_report(report);
            return;
        }
        if matches!(save.owner, SaveOwner::TableRename) {
            self.table_rename_save_report(report);
            return;
        }
        if matches!(save.owner, SaveOwner::TableCreation) {
            self.table_creation_save_report(report);
            return;
        }
        let outcome = match (save.owner, report.outcome) {
            (SaveOwner::Insert, SaveOutcome::Saved { receipt })
                if matches!(
                    receipt.known,
                    SaveKnownOutcome::RolledBack | SaveKnownOutcome::OriginalVerified
                ) =>
            {
                EditSaveOutcome::Failed(known_result(&self.state.localizer, Some(receipt.known)))
            }
            _ => save_outcome(&self.state.localizer, report.outcome),
        };
        match save.owner {
            SaveOwner::Cell => self.state.set_edit_save_outcome(report.id, outcome),
            SaveOwner::Insert => self.state.set_insert_save_outcome(report.id, outcome),
            SaveOwner::Delete => self.state.set_delete_save_outcome(report.id, outcome),
            SaveOwner::QueryAction => {
                self.state.set_query_action_save_outcome(report.id, outcome);
            }
            SaveOwner::TableCreation => {}
            SaveOwner::TableRename | SaveOwner::FieldAddition => {}
            SaveOwner::QueryDefinition => {}
        }
    }

    pub(super) fn cancel_current_save(&mut self) {
        if self.cancel_current_field_addition_operation() {
            return;
        }
        if self.cancel_query_definition_operation() {
            return;
        }
        if let Some(id) = self.editing.save.as_ref().map(|save| save.id) {
            self.cancel_save(id);
        } else {
            self.cancel_active_recovery();
        }
    }

    pub(super) fn cancel_save(&mut self, id: u64) {
        self.note_field_addition_cancellation(id);
        if self.editing.save.as_ref().is_some_and(|save| save.id == id) {
            self.note_query_definition_save_cancellation(id);
            self.editing.worker.cancel(id);
        }
    }

    pub(super) fn resolve_save(&mut self, id: u64, recover: bool) {
        if let Some(identity) = self
            .editing
            .field_addition
            .as_ref()
            .and_then(|host| host.identity)
            .filter(|identity| identity.save_id == id)
        {
            self.field_addition_save_command(
                identity,
                if recover {
                    tablecraft_ui::FieldAdditionSaveCommand::Recover
                } else {
                    tablecraft_ui::FieldAdditionSaveCommand::Release
                },
            );
            return;
        }
        if self.editing.query_definition.is_some()
            && !self.can_resolve_query_definition_save(id, recover)
        {
            return;
        }
        let Some(save) = &mut self.editing.save else {
            return;
        };
        if save.id != id
            || matches!(save.owner, SaveOwner::QueryAction)
                && !self.state.can_resolve_query_action_save(id, recover)
            || matches!(save.owner, SaveOwner::TableRename)
                && !self.state.can_resolve_table_rename_save(id, recover)
        {
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
                    SaveOwner::QueryAction => {
                        self.state.begin_query_action_resolution(id, next);
                    }
                    SaveOwner::TableRename => {
                        self.state.begin_table_rename_resolution(id, next);
                    }
                    SaveOwner::QueryDefinition => {
                        self.query_definition_resolution_started(next);
                    }
                    SaveOwner::FieldAddition => {}
                    SaveOwner::TableCreation => {}
                }
            }
            Err(error) => self.state.error = Some(error.to_string()),
        }
    }

    pub(super) fn acknowledge_save(&mut self, id: u64) {
        if let Some(identity) = self
            .editing
            .field_addition
            .as_ref()
            .and_then(|host| host.identity)
            .filter(|identity| identity.save_id == id)
        {
            self.acknowledge_field_addition(identity);
            return;
        }
        if self.editing.query_definition.is_some() {
            self.acknowledge_query_definition(id);
            return;
        }
        if self
            .editing
            .save
            .as_ref()
            .is_some_and(|save| matches!(save.owner, SaveOwner::TableRename))
        {
            self.acknowledge_table_rename(id);
            return;
        }

        let Some(save) = self.editing.save.as_ref().filter(|save| save.id == id) else {
            return;
        };
        if matches!(save.owner, SaveOwner::Insert) && !self.state.can_acknowledge_insert_save(id) {
            return;
        }
        if matches!(save.owner, SaveOwner::QueryAction)
            && !self.state.can_acknowledge_query_action_save(id)
        {
            return;
        }
        let refresh_after_ack = match save.owner {
            SaveOwner::Insert => matches!(save.report.map(|report| report.outcome),
                Some(SaveOutcome::Saved { receipt })
                    if matches!(receipt.known, SaveKnownOutcome::Committed | SaveKnownOutcome::ReplacementVerified)),
            _ => matches!(
                save.report.map(|report| report.outcome),
                Some(SaveOutcome::Saved { .. })
            ),
        };
        let workspace_restore = if refresh_after_ack {
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
                let permission = match &save.insert_retry {
                    Some(retry) if retry.evidence.can_retry_original() => {
                        InsertRetryPermission::OriginalImage
                    }
                    _ => InsertRetryPermission::Blocked,
                };
                self.state.acknowledge_insert_save(id, permission);
                if !refresh_after_ack {
                    self.editing.insert_retry =
                        save.insert_retry.map(|context| RetainedInsertRetry {
                            save_id: id,
                            context,
                        });
                }
            }
            SaveOwner::Delete => {
                self.state.close_delete();
            }
            SaveOwner::QueryAction => {
                self.state.acknowledge_query_action_save(id);
            }
            SaveOwner::TableCreation => {}
            SaveOwner::TableRename | SaveOwner::FieldAddition => {}
            SaveOwner::QueryDefinition => {}
        }
        if refresh_after_ack {
            self.select_file(save.path);
            self.open();
            if let Some(restore) = workspace_restore {
                self.retain_workspace_restore(restore);
            }
            self.editing.refresh = Some((self.generation, save.table, save.offset));
            self.editing.refresh_query_action = matches!(save.owner, SaveOwner::QueryAction);
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
            if let Some(target) = &mut self.close_after_first_record
                && matches!(&target.kind, ObjectKind::Table { name } if *name == table)
                && let Some(database) = &self.state.database
            {
                // This exact acknowledged save's fresh Open advances the
                // document revision. Keep the original close bound to its
                // actual saved table, rather than an obsolete tab revision.
                target.document_revision = database.document_revision;
            }
            if let Some(database) = &self.state.database {
                self.state
                    .set_save_cleanup(database.document_revision, cleanup);
            }
            let restored = if std::mem::take(&mut self.editing.refresh_query_action) {
                self.restore_workspace_after_query_action()
            } else {
                self.restore_workspace_after_save(&table)
            };
            if !restored {
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
        } else if self.state.insertion_dialog_open() || self.state.insert_continuation_pending() {
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

#[cfg(all(test, any(target_os = "macos", target_os = "linux", windows)))]
mod tests;
