//! Local presentation responses. No action escapes to a runtime host.
use super::{Preview, fixtures};
use crate::workspace::editing::EditDialog;
use crate::workspace::objects::{ObjectKind, ObjectOpen, ObjectTarget};
use crate::{
    Action, Cell, CellKind, CollectionKind, EditDraft, EditInput, EditReview, EditSaveOutcome,
    EditTarget, SavedQueryTarget,
};

impl Preview {
    pub(super) fn dispatch(&mut self, action: Action) {
        let save_owner = self.state.query_action_save_pending()
            || matches!(
                self.state.edit,
                Some(EditDialog::Saving { .. } | EditDialog::Finished { .. })
            )
            || matches!(
                self.state.insertion,
                Some(
                    crate::workspace::insertion::Dialog::Saving { .. }
                        | crate::workspace::insertion::Dialog::Finished { .. }
                )
            )
            || matches!(
                self.state.deletion,
                Some(
                    crate::workspace::deletion::Dialog::Saving { .. }
                        | crate::workspace::deletion::Dialog::Finished { .. }
                )
            );
        if save_owner
            && !matches!(
                &action,
                Action::CancelEditSave { .. }
                    | Action::RecoverEditSave { .. }
                    | Action::ReleaseEditSave { .. }
                    | Action::AcknowledgeEditSave { .. }
                    | Action::Cancel
            )
        {
            return;
        }

        if self.query_action_save_action(&action)
            || self.deletion_save_action(&action)
            || self.insertion_save_action(&action)
        {
            return;
        }
        match action {
            Action::RetryInsert { save_id } => {
                // Illustrate a refused synthetic read; this host has no witness,
                // database source or native capability to reopen.
                let operation = self.request();
                if self.state.begin_insert_retry(save_id, operation) {
                    self.state.fail_insert_retry(
                        operation,
                        self.state.localizer.text("insertion-mock-help"),
                    );
                }
            }
            Action::CancelInsertRetry { operation_id } => {
                self.state.fail_insert_retry(
                    operation_id,
                    self.state.localizer.text("insertion-mock-help"),
                );
            }
            Action::DiscardRetainedInsert { save_id } => {
                self.state.discard_retained_insert(save_id);
            }
            Action::BeginRecovery { attempt_id } => {
                let operation = self.request();
                if self.state.begin_recovery(attempt_id, operation) {
                    let message = self.restart_result_message();
                    self.state
                        .set_recovery_outcome(operation, crate::RecoveryOutcome::Finished(message));
                }
            }
            Action::CloseRecovery { attempt_id } => {
                self.state.close_recovery(attempt_id);
            }
            Action::AcknowledgeRecovery { operation_id } => {
                self.state.acknowledge_recovery(operation_id);
            }
            Action::CancelRecovery { .. } | Action::ResolveRecovery { .. } => {}
            Action::SaveTableDraft { request } => {
                if self.state.begin_table_draft_save(&request) {
                    self.state.fail_table_draft_save(
                        request.id,
                        self.state.localizer.text("table-draft-save-unavailable"),
                    );
                }
            }
            Action::CancelTableDraftSave { id } => {
                self.state.cancel_table_draft_save(id);
            }
            Action::RecoverTableDraftSave { .. } | Action::ReleaseTableDraftSave { .. } => {}
            Action::AcknowledgeTableDraftSave { id } => {
                self.state.acknowledge_table_draft_save(id);
            }
            Action::TableDraftsDiscarded | Action::TableDraftDiscardCancelled => {}
            Action::DescribeQueryDesign { request, table } => {
                self.mock_describe_query_design(request, table)
            }
            Action::BuildQueryDesign {
                request,
                draft,
                completion,
            } => {
                let id = self.request();
                if self
                    .state
                    .begin_query_design_build(request, draft, completion, id)
                {
                    self.state
                        .fail_query_design(id, self.state.localizer.text("qbe-mock-unavailable"));
                }
            }
            Action::ImportQueryDesign { request, sql } => {
                let id = self.request();
                if self.state.begin_query_design_import(request, sql, id) {
                    self.state
                        .fail_query_design(id, self.state.localizer.text("qbe-mock-unavailable"));
                }
            }
            Action::CancelQueryDesign { request_id } => {
                self.state.cancel_query_design(request_id);
            }
            Action::DescribeTotals { target } => self.mock_describe_totals(target),
            Action::ComputeTotals { target, specs } => self.mock_compute_totals(target, specs),
            Action::CancelTotals { request_id } => {
                self.state.cancel_totals(request_id);
            }
            Action::DescribeFieldFilter { target } => self.mock_describe_field_filter(target),
            Action::ApplyFieldFilter {
                target,
                operator,
                inputs,
            } => self.mock_apply_field_filter(target, operator, inputs),
            Action::CloseFieldFilter { request_id } => {
                self.state.close_field_filter(request_id);
            }
            Action::FilterSelection { target, comparison } => {
                let request = self.request();
                if self
                    .state
                    .begin_filter_selection(request, target, comparison)
                {
                    self.state.fail_filter(
                        request,
                        self.state.localizer.text("filtering-mock-unavailable"),
                    );
                }
            }
            Action::FilterPage {
                target,
                offset,
                enabled,
            } => {
                let request = self.request();
                if self
                    .state
                    .begin_filter_page(request, target, offset, enabled)
                {
                    self.state.fail_filter(
                        request,
                        self.state.localizer.text("filtering-mock-unavailable"),
                    );
                }
            }
            Action::ClearTableFilter { target } => {
                let request = self.request();
                if self.state.begin_clear_filter(request, target) {
                    self.state.fail_filter(
                        request,
                        self.state.localizer.text("filtering-mock-unavailable"),
                    );
                }
            }
            Action::ReplaceFindMatch { request } => {
                let request_id = self.request();
                if self.state.begin_replace_find(request_id, request) {
                    self.state.fail_replace_find(
                        request_id,
                        self.state.localizer.text("find-replace-mock-help"),
                    );
                }
            }
            Action::FindNext { request } => self.mock_find(request),
            Action::CancelFind { .. } => {}
            Action::ReviewDelete { target } => self.mock_review_delete(target, true),
            Action::ApplyDelete { review_id } => self.mock_apply_delete(review_id),
            Action::CloseDelete { request_id } => self.mock_close_delete(request_id),
            Action::ChooseCreationLocation => {
                if self.state.creation_available {
                    self.state
                        .set_creation_destination("/preview/Database1.accdb".into());
                }
            }
            Action::CreateDatabase { request } => {
                if self.state.begin_database_creation(&request) {
                    self.state.finish_database_creation(
                        request.id,
                        crate::workspace::CreationOutcome::NotPublished,
                    );
                }
            }
            Action::ChooseFile | Action::Open => {
                self.state.password.clear();
                self.state.error = Some(self.state.localizer.text("settings-mock-open-disabled"));
                self.state.show_open();
            }
            Action::Page { table, offset } => {
                let target = ObjectTarget {
                    document_revision: fixtures::REVISION,
                    kind: ObjectKind::Table {
                        name: table.clone(),
                    },
                };
                if self.state.prepare_object_open(target).is_some() {
                    self.state.error = None;
                    self.state.set_table_data(fixtures::page(&table, offset));
                }
            }
            Action::ViewSchema { table } => {
                let target = ObjectTarget {
                    document_revision: fixtures::REVISION,
                    kind: ObjectKind::Table {
                        name: table.clone(),
                    },
                };
                if self.state.prepare_object_open(target).is_some() {
                    self.state.error = None;
                    self.state.prepare_schema_view();
                    self.state.set_schema(fixtures::schema(&table));
                }
            }
            Action::CancelTableRenameReview { request_id } => {
                self.state.cancel_table_rename_review(request_id);
            }
            Action::RenameTable { request } => {
                self.state.fail_table_rename(
                    request.id,
                    self.state.localizer.text("navigation-rename-unavailable"),
                );
            }
            Action::ApplyQueryAction { review_id } => self.mock_apply_query_action(review_id),
            Action::CancelQueryActionReview { request_id } => {
                self.mock_cancel_query_action_review(request_id);
            }
            Action::Query => {
                if self.mock_review_query_action().is_some() {
                    return;
                }
                self.mock_open_object(ObjectTarget {
                    document_revision: fixtures::REVISION,
                    kind: ObjectKind::Sql,
                });
                self.state.error = None;
                if self.state.sql == fixtures::SQL {
                    self.state
                        .set_query_data(Some(fixtures::SQL.into()), fixtures::query());
                } else {
                    self.state.error =
                        Some(self.state.localizer.text("settings-mock-sql-disabled"));
                }
            }
            Action::RunParameterizedQuery {
                inspection_id,
                parameters,
            } => {
                if self.state.can_run_parameterized_query(
                    inspection_id,
                    &self.state.sql,
                    &parameters,
                ) {
                    self.state.clear_query_parameter_acquisition();
                    self.state.error = Some(self.state.localizer.text("query-mock-error"));
                }
            }
            Action::ReviewSavedQuerySql { request } => self.refuse_saved_sql_save(request),
            Action::SavedQuerySqlClose { result } => self.finish_saved_sql_close(result),
            // This synthetic host never owns a native save or read-reopen witness.
            Action::SavedQuerySqlSave { .. } => {}
            Action::InspectSavedQuery { target } => self.inspect_saved(target),
            Action::RunSavedQuery { target, parameters } => {
                if self.state.can_run_saved_query(&target, &parameters) {
                    self.state.error = None;
                    let request = self.request();
                    self.state
                        .begin_saved_query_run(target.clone(), parameters, request);
                    let mut data = fixtures::query();
                    data.table = target.name.clone();
                    self.state.set_saved_query_data(request, target, data);
                }
            }
            Action::NavigateRecord { target } => {
                if self.state.can_navigate_record(&target) {
                    self.state.error = None;
                    let request = self.request();
                    self.state.begin_record_navigation(target.clone(), request);
                    self.state.set_record_data(
                        request,
                        target.clone(),
                        fixtures::page(&target.table, target.parent_ordinal),
                    );
                }
            }
            Action::ViewContents { target } => {
                let kind = match target.column.as_str() {
                    "Categories" => CollectionKind::Multivalue,
                    "Notes" => CollectionKind::VersionHistory,
                    _ => CollectionKind::Attachments,
                };
                self.collection(target, kind);
            }
            Action::CloseContents { request_id } => {
                if self.state.contents_request_id() == Some(request_id) {
                    self.state.close_contents();
                }
            }
            Action::OpenObject { target } => self.mock_open_object(target),
            Action::ActivateObject { tab } => {
                if self.state.activate_object(&tab)
                    && let Some(open) = self.state.active_object_load()
                {
                    self.mock_load_object(open);
                }
            }
            Action::CloseObject { tab } => {
                self.state.close_object(&tab);
            }
            Action::RetryObject { tab } => {
                if self.state.can_retry_object(&tab)
                    && let Some(open) = self.state.active_object_load()
                {
                    self.state.error = None;
                    self.mock_load_object(open);
                }
            }
            Action::ViewObjectDatasheet { tab } => {
                if let Some(open) = self.state.prepare_object_datasheet(&tab)
                    && open.needs_load
                {
                    self.mock_load_object(open);
                }
            }
            Action::SortView {
                target,
                sort,
                enabled,
            } => {
                let request = self.request();
                if self.state.begin_sort_view(request, target, sort, enabled) {
                    self.state.fail_filter(
                        request,
                        self.state.localizer.text("filtering-sort-mock-unavailable"),
                    );
                }
            }
            Action::BeginInsert { target } => self.mock_begin_insert(target),
            Action::ReviewInsert { draft_id, inputs } => self.mock_review_insert(draft_id, inputs),
            Action::ApplyInsert { review_id } => self.mock_apply_insert(review_id),
            Action::CloseInsert {
                request_id,
                draft_id,
            } => self.mock_close_insert(request_id, draft_id),
            Action::BeginEdit { target } => self.edit(target),
            Action::PasteCell { target, text } => {
                if self.state.can_paste_text(&target, &text) {
                    self.edit(target);
                    if let Some(EditDialog::Draft { input, .. }) = &mut self.state.edit {
                        *input = crate::EditInput {
                            text,
                            is_null: false,
                        };
                    }
                }
            }
            Action::ReviewEdit { draft_id, input } => {
                if self.state.can_review_edit(draft_id, &input) {
                    self.preview_review(input);
                }
            }
            Action::ApplyEdit { review_id } => {
                if self.state.can_apply_edit(review_id) {
                    let save_id = self.request();
                    self.state.begin_edit_save(review_id, save_id);
                    self.state.set_edit_save_outcome(
                        save_id,
                        EditSaveOutcome::Saved(self.state.localizer.text("settings-mock-saved")),
                    );
                }
            }
            Action::CloseEdit {
                request_id,
                draft_id,
            } => {
                let matches = match &self.state.edit {
                    Some(EditDialog::Checking {
                        request_id: current,
                        ..
                    }) => *current == request_id && draft_id.is_none(),
                    Some(EditDialog::Draft {
                        request_id: current,
                        draft,
                        ..
                    }) => *current == request_id && draft_id == Some(draft.id),
                    _ => false,
                };
                if matches {
                    self.state.close_edit();
                }
            }
            Action::AcknowledgeEditSave { save_id } => {
                let matches_current_terminal = match &self.state.edit {
                    Some(EditDialog::Finished {
                        save_id: current,
                        outcome,
                    }) => {
                        *current == save_id && !matches!(outcome, EditSaveOutcome::Retained { .. })
                    }
                    _ => false,
                };
                if matches_current_terminal {
                    self.state.close_edit();
                }
            }
            Action::CancelEditSave { save_id } => self.request_mock_cancellation(save_id),
            Action::RecoverEditSave { save_id } => self.resolve_mock_save(save_id, true),
            Action::ReleaseEditSave { save_id } => self.resolve_mock_save(save_id, false),
            Action::Cancel if save_owner => {
                if let Some(EditDialog::Saving { save_id, .. }) = &self.state.edit {
                    self.request_mock_cancellation(*save_id);
                }
            }
            Action::Cancel => {
                self.state.clear_query_parameter_acquisition();
                self.state.interrupt_saved_query();
                self.state.busy = false;
                self.state.error = Some(self.state.localizer.text("settings-mock-cancelled"));
            }
        }
    }

    fn request_mock_cancellation(&mut self, save_id: u64) {
        if let Some(EditDialog::Saving {
            save_id: current,
            cancel_requested,
            ..
        }) = &mut self.state.edit
            && *current == save_id
        {
            // An accepted request is not a rollback or a released save owner.
            *cancel_requested = true;
        }
    }

    fn resolve_mock_save(&mut self, save_id: u64, recover: bool) {
        let Some(EditDialog::Finished {
            save_id: current,
            outcome:
                EditSaveOutcome::Retained {
                    can_recover,
                    can_release,
                    ..
                },
        }) = &self.state.edit
        else {
            return;
        };
        if *current != save_id {
            return;
        }
        let allowed = if recover { *can_recover } else { *can_release };
        if !allowed {
            return;
        }

        let operation_id = self.request();
        self.state.begin_edit_resolution(save_id, operation_id);
        let message = self.state.localizer.text("settings-mock-resolved");
        let outcome = if recover {
            EditSaveOutcome::Saved(message)
        } else {
            // The native host also keeps released resources separate from Saved.
            EditSaveOutcome::Failed(message)
        };
        self.state.set_edit_save_outcome(operation_id, outcome);
    }

    pub(super) fn mock_open_object(&mut self, target: ObjectTarget) {
        if let Some(open) = self.state.prepare_object_open(target)
            && open.needs_load
        {
            self.mock_load_object(open);
        }
    }

    fn mock_load_object(&mut self, open: ObjectOpen) {
        match open.tab.target.kind {
            ObjectKind::Table { name } => {
                if open.table_design {
                    self.state.prepare_schema_view();
                    self.state.set_schema(fixtures::schema(&name));
                } else {
                    self.state
                        .set_table_data(fixtures::page(&name, open.offset));
                }
            }
            ObjectKind::SavedQuery { object_id, name } => {
                self.inspect_saved(SavedQueryTarget {
                    document_revision: open.tab.target.document_revision,
                    object_id,
                    name,
                });
            }
            ObjectKind::DraftTable { .. } | ObjectKind::Sql => {}
        }
    }

    pub(super) fn inspect_saved(&mut self, target: SavedQueryTarget) {
        if !self.state.can_inspect_saved_query(&target) {
            return;
        }
        self.state.error = None;
        let request = self.request();
        self.state
            .begin_saved_query_inspection(target.clone(), request);
        let parameters = fixtures::parameters(&target);
        self.state
            .set_saved_query_details(request, target, parameters);
    }

    pub(super) fn edit(&mut self, target: EditTarget) {
        if !self.state.can_edit_cell(&target) {
            return;
        }
        let Some(data) = &self.state.data else {
            return;
        };
        let Some(column) = data.column_index(&target.column) else {
            return;
        };
        let Some(row) = data.row_index(target.parent_ordinal) else {
            return;
        };
        let Some(original) = data.rows.get(row).and_then(|row| row.get(column)) else {
            return;
        };
        let original = copy_cell(original);
        let initial = EditInput {
            text: if target.column == "Balance" {
                "1245.50".into()
            } else {
                original.text.clone()
            },
            is_null: matches!(original.kind, CellKind::Scalar { is_null: true, .. }),
        };
        let request = self.request();
        self.state.begin_edit(target.clone(), request);
        self.state.set_edit_draft(
            request,
            EditDraft {
                id: request,
                target,
                original,
                initial,
                input_hint: crate::InputHint::Text,
                nullable: true,
            },
        );
    }

    pub(super) fn preview_review(&mut self, input: EditInput) -> Option<u64> {
        let Some(EditDialog::Draft {
            draft,
            input: current,
            ..
        }) = &mut self.state.edit
        else {
            return None;
        };
        *current = input.clone();
        let draft_id = draft.id;
        let target = draft.target.clone();
        let before = copy_cell(&draft.original);
        let after = if input.is_null {
            fixtures::null()
        } else {
            let mut after = fixtures::text(&input.text);
            after.alignment = before.alignment;
            after
        };
        let request = self.request();
        self.state.begin_edit_review(draft_id, input, request);
        self.state.set_edit_review(
            request,
            EditReview {
                id: request,
                draft_id,
                target,
                before,
                after,
                changed_rows: 1,
            },
        );
        Some(request)
    }
}

pub(super) fn copy_cell(cell: &Cell) -> Cell {
    Cell {
        presentation: cell.presentation,
        alignment: cell.alignment,
        text: cell.text.clone(),
        kind: cell.kind,
    }
}
