#[cfg(test)]
mod browser_acceptance;
mod creating;
mod editing;
mod filtering;
mod find;
mod navigation;
mod objects;
mod opening;
mod query_contents;
mod query_design;
mod query_limits;
mod query_parameters;
mod read_failures;
mod read_requests;
mod replacing;
mod saved;
mod table_drafts;
#[cfg(test)]
mod test_support;
mod totals;
mod updating;

use crate::file_dialog::{FileDialog, SelectedFile};
use crate::font_inventory::FontInventory;
use crate::updating::{RetainedOutcome, UpdateWorker};
use creating::Creating;
use editing::Editing;
use editing::table_creation::TableCreationHost;
use editing::table_rename::TableRenameHost;
use read_requests::request_read;
use std::{path::PathBuf, sync::Arc};
use table_drafts::DeferredDraftAction;
use tablecraft_runtime::{
    Command, ContentsTarget as RuntimeContentsTarget, CreationWorker, FilterHandle,
    QueryRunOptions, Response, SaveWorker, Worker,
};
use tablecraft_ui::{
    Action, ContentsTarget, Data, Database, FieldFilterTarget, FindPosition, FindRequest,
    ObjectKind, ObjectTarget, QueryDesignRequest, RecordTarget, SavedQuery, SavedQuerySqlRequest,
    SavedQueryTarget, Schema, State, Table, TotalSpec, TotalsTarget,
};
use tablecraft_update::session::UpdateCommand;
use zeroize::Zeroize;

enum PendingOperation {
    TableDropInspection {
        request_id: u64,
    },
    TableDropReview {
        request_id: u64,
    },
    TableDropReopen {
        request_id: u64,
    },
    DatabaseOptionsInspection {
        revision: u64,
        request_id: u64,
    },
    DatabaseOptionsReview {
        request_id: u64,
    },
    DatabaseOptionsReopen {
        request_id: u64,
    },
    FieldDropInspection {
        request_id: u64,
    },
    FieldDropReview {
        request_id: u64,
    },
    FieldDropReopen {
        request_id: u64,
    },
    FieldRenameInspection {
        request_id: u64,
    },
    FieldRenameReview {
        request_id: u64,
    },
    FieldRenameReopen {
        request_id: u64,
    },
    FieldAdditionAvailability {
        request_id: u64,
        tab: tablecraft_ui::ObjectTab,
    },
    FieldAdditionReview {
        request_id: u64,
    },
    FieldAdditionReopen {
        request_id: u64,
    },
    QueryDefinitionReview {
        request_id: u64,
    },
    QueryDefinitionReopen {
        request_id: u64,
    },
    TableRenameReview {
        request_id: u64,
    },
    RenamedTableReopen {
        request_id: u64,
    },
    QueryActionReview {
        request_id: u64,
    },
    QuerySchemaReopen {
        request_id: u64,
    },
    Read,
    Open {
        request_id: u64,
        path: PathBuf,
    },
    CreatedOpen,
    TableCreationReview {
        request_id: u64,
    },
    CreatedTableReopen {
        request_id: u64,
    },
    Query {
        request_id: u64,
    },
    QueryDiscovery {
        request_id: u64,
        inspection_id: u64,
        options: QueryRunOptions,
    },
    QueryInspection {
        request_id: u64,
        options: QueryRunOptions,
    },
    QueryDesignDescribe {
        request_id: u64,
        request: QueryDesignRequest,
        table: String,
    },
    QueryDesignBuild {
        request_id: u64,
        options: QueryRunOptions,
        request: QueryDesignRequest,
    },
    QueryDesignImport {
        request_id: u64,
        request: QueryDesignRequest,
    },
    QueryDesignRun {
        request_id: u64,
    },
    TotalsDescribe {
        request_id: u64,
        target: TotalsTarget,
    },
    TotalsCompute {
        request_id: u64,
        target: TotalsTarget,
        specs: Vec<TotalSpec>,
    },
    SortView {
        request_id: u64,
    },
    FilterSelection {
        request_id: u64,
    },
    FieldFilterDescribe {
        request_id: u64,
        target: FieldFilterTarget,
    },
    FieldFilterApply {
        request_id: u64,
    },
    FilterPage {
        request_id: u64,
    },
    FilterRestore {
        request_id: u64,
    },
    ClearFilter {
        request_id: u64,
    },
    Find {
        request_id: u64,
        request: FindRequest,
        filter: Option<find::FilterWitness>,
    },
    FindPage {
        request_id: u64,
        request: FindRequest,
        position: FindPosition,
        filter: Option<find::FilterWitness>,
    },
    ReplaceFind {
        request_id: u64,
    },
    SavedInspection {
        target: SavedQueryTarget,
        request_id: u64,
    },
    SavedRun {
        target: SavedQueryTarget,
        request_id: u64,
    },
    SavedSqlInspection {
        request_id: u64,
        request: SavedQuerySqlRequest,
    },
    Record {
        target: RecordTarget,
        request_id: u64,
    },
    QueryContents {
        request_id: u64,
        target: tablecraft_ui::QueryContentsTarget,
    },
    Contents {
        target: ContentsTarget,
        request_id: u64,
    },
    Edit {
        request_id: u64,
    },
    Insert {
        request_id: u64,
    },
    InsertRetry {
        request_id: u64,
        save_id: u64,
    },
    Delete {
        request_id: u64,
    },
}

pub(super) struct Desktop {
    state: State,
    worker: Worker,
    generation: u64,
    pending: PendingOperation,
    file_dialog: FileDialog,
    file_dialog_purpose: FileDialogPurpose,
    selected_file: Option<SelectedFile>,
    creating: Creating,
    deferred_draft_action: Option<DeferredDraftAction>,
    table_creation: Option<TableCreationHost>,
    table_rename: Option<TableRenameHost>,
    resume_after_table_creation: bool,
    resume_after_query_definition: bool,
    resume_after_field_schema: bool,
    close_after_first_record: Option<ObjectTarget>,
    editing: Editing,
    recovery: Option<editing::recovery::RecoveryHost>,
    recovery_reopen: Option<editing::recovery::RecoveryReopen>,
    filter: Option<FilterHandle>,
    objects: objects::Sessions,
    query_parameters: Option<query_parameters::Inspected>,
    query_contents: query_contents::Publications,
    query_run_options: QueryRunOptions,
    startup_visibility_pending: bool,
    font_inventory: FontInventory,
    updates: UpdateWorker,
    update_outcomes: Vec<Arc<RetainedOutcome>>,
    update_restart_pending: bool,
    update_exit_requested: bool,
}

#[derive(Default)]
enum FileDialogPurpose {
    #[default]
    Open,
    Creation,
}

impl Desktop {
    pub(super) fn new(ctx: egui::Context, path: Option<PathBuf>) -> Self {
        // NO_CLONE: Editing's long-lived SaveWorker repaint closure owns its event-loop handle
        // independently of Creating and the read worker. Context shares egui state rather
        // than copying the UI.
        let editing = Editing::new(ctx.clone());
        // NO_CLONE: Creating's long-lived creation-worker callback owns its event-loop handle
        // independently of Editing and the read worker. Context shares egui state rather
        // than copying the UI.
        let creating = Creating::new(ctx.clone());
        // NO_CLONE: the independent update worker owns a shared repaint handle;
        // egui Context shares event-loop state rather than duplicating the UI.
        let update_context = ctx.clone();
        let updates = UpdateWorker::new(move || update_context.request_repaint());
        let worker = Worker::new(move || ctx.request_repaint());
        let mut app = Self {
            state: State::default(),
            worker,
            generation: 0,
            pending: PendingOperation::Read,
            file_dialog: FileDialog::default(),
            file_dialog_purpose: FileDialogPurpose::default(),
            selected_file: None,
            creating,
            deferred_draft_action: None,
            table_creation: None,
            table_rename: None,
            resume_after_table_creation: false,
            resume_after_query_definition: false,
            resume_after_field_schema: false,
            close_after_first_record: None,
            editing,
            recovery: None,
            recovery_reopen: None,
            filter: None,
            objects: objects::Sessions::default(),
            query_parameters: None,
            query_contents: query_contents::Publications::default(),
            query_run_options: QueryRunOptions::default(),
            startup_visibility_pending: true,
            font_inventory: FontInventory::default(),
            updates,
            update_outcomes: Vec::new(),
            update_restart_pending: false,
            update_exit_requested: false,
        };
        app.state
            .set_query_run_limits(query_limits::ui_limits(app.query_run_options));
        app.state.file_picker_available = true;
        app.state
            .set_updates_available(tablecraft_update::platform::Platform::detect().is_some());
        app.state.creation_available = CreationWorker::is_supported();
        app.state.table_save_available = SaveWorker::supports_table_creation();
        app.state.table_rename_available = SaveWorker::supports_table_rename();
        app.state.field_addition_available = SaveWorker::supports_field_addition();
        app.state.field_rename_available = SaveWorker::supports_field_rename();
        app.state.field_drop_available = SaveWorker::supports_field_drop();
        app.state.table_drop_available = SaveWorker::supports_table_drop();
        app.state.database_options_available = SaveWorker::supports_database_options();
        app.state.saved_query_sql_edit_available = SaveWorker::supports_query_sql();
        app.state.find_available = true;
        app.state.find_formatted_available = true;
        app.state.replace_available = cfg!(any(target_os = "macos", target_os = "linux", windows));
        app.state.formatting.system_locale = crate::regional::locale();
        if let Some(path) = path {
            app.select_file(path);
            app.open();
        }
        app
    }

    fn submit(&mut self, command: Command) -> bool {
        if self.editing.has_save()
            || self.creating.is_busy()
            || self.recovery.is_some()
            || self.state.table_draft_field_delete_open()
        {
            return false;
        }
        if self.state.table_draft_save_open()
            && !matches!(
                &command,
                Command::ReviewTableCreation { .. } | Command::OpenTableCreated { .. }
            )
        {
            return false;
        }
        self.query_contents.prune(&self.state);
        let query_contents = match &command {
            Command::InspectQueryCollection { result, binding } => {
                let Some(target) = self.query_contents_target(result, *binding) else {
                    return false;
                };
                Some(target)
            }
            _ => None,
        };
        let contents = match &command {
            Command::InspectCollection { target } => {
                Some(crate::view_models::contents_target(target))
            }
            _ => None,
        };
        if let Some(target) = &contents {
            if !self.state.can_view_contents(target) {
                self.state.error = Some(self.state.localizer.text("collections-stale-view"));
                return false;
            }
        } else if query_contents.is_none() {
            self.state.close_contents();
        }
        if matches!(&command, Command::Describe { .. }) {
            self.state.prepare_schema_view();
        }
        let query_sql = if let Command::Query { sql, .. } = &command {
            let Some(captured) = self.state.capture_query_sql(sql) else {
                return false;
            };
            Some(captured)
        } else {
            None
        };
        let created_open = matches!(&command, Command::OpenCreated { .. });
        let opening_path = match &command {
            // NO_CLONE: Command is consumed by the read worker; host Open pending state retains the
            // exact attempted filesystem path for failure/recovery identity.
            Command::Open { path, .. } => Some(path.clone()),
            _ => None,
        };
        self.clear_query_parameters();
        let Some(generation) = request_read(command, &self.worker, &mut self.state) else {
            self.read_request_rejected();
            return false;
        };
        self.generation = generation;
        self.state.clear_query_run();
        self.pending = if let Some(target) = query_contents {
            self.state.begin_query_contents(target, self.generation);
            PendingOperation::QueryContents {
                target,
                request_id: self.generation,
            }
        } else if let Some(target) = contents {
            // NO_CLONE: The collection dialog and host pending record each retain exact request
            // target identity for late-response and close/cancellation checks.
            self.state.begin_contents(target.clone(), self.generation);
            PendingOperation::Contents {
                target,
                request_id: self.generation,
            }
        } else if let Some(sql) = query_sql {
            if !self.state.begin_query_run(self.generation, sql) {
                self.worker.cancel();
                self.state.busy = false;
                return false;
            }
            PendingOperation::Query {
                request_id: self.generation,
            }
        } else if created_open {
            PendingOperation::CreatedOpen
        } else if let Some(path) = opening_path {
            PendingOperation::Open {
                request_id: self.generation,
                path,
            }
        } else {
            PendingOperation::Read
        };
        self.state.busy = true;
        self.state.error = None;
        true
    }

    fn select_file(&mut self, path: PathBuf) {
        let selected = SelectedFile::new(path);
        self.state.path = selected.display().to_owned();
        self.selected_file = Some(selected);
    }

    fn poll_file_dialog(&mut self) {
        if let Some(selection) = self.file_dialog.poll() {
            match std::mem::take(&mut self.file_dialog_purpose) {
                FileDialogPurpose::Open => self.file_selected(selection),
                FileDialogPurpose::Creation => self.creation_selected(selection),
            }
        }
    }

    fn file_selected(&mut self, selection: Option<PathBuf>) {
        if self.editing.has_save()
            || self.creating.is_busy()
            || self.recovery.is_some()
            || self.state.table_draft_save_open()
            || self.state.table_draft_field_delete_open()
            || self.state.insertion_dialog_open()
            || self.state.insert_continuation_pending()
        {
            return;
        }
        if let Some(path) = selection {
            self.select_file(path);
            self.open();
        }
    }

    fn open(&mut self) {
        if self.editing.has_save()
            || self.creating.is_busy()
            || self.recovery.is_some()
            || self.state.table_draft_save_open()
            || self.state.table_draft_field_delete_open()
        {
            return;
        }
        let path = match self
            .selected_file
            .as_ref()
            .and_then(|selected| selected.path_for(&self.state.path))
        {
            Some(path) => path.to_path_buf(),
            None => {
                self.selected_file = None;
                PathBuf::from(&self.state.path)
            }
        };
        let password = std::mem::take(&mut self.state.password);
        self.open_request(Command::Open { path, password });
    }

    fn open_request(&mut self, command: Command) {
        if self.editing.has_save()
            || self.creating.is_busy()
            || self.recovery.is_some()
            || self.state.insertion_dialog_open()
            || self.state.insert_continuation_pending()
            || self.state.table_draft_field_delete_open()
        {
            return;
        }
        let Some(DeferredDraftAction::Open(command)) =
            self.guard_table_draft_action(DeferredDraftAction::Open(Box::new(command)))
        else {
            return;
        };
        self.editing.clear_review();
        self.editing.clear_refresh();
        self.state.show_open();
        self.state.database = None;
        self.filter = None;
        self.objects = objects::Sessions::default();
        self.state.data = None;
        self.state.query_data = None;
        self.state.clear_schema();
        self.state.close_contents();
        self.recovery_reopen = None;
        self.submit(*command);
    }

    fn poll(&mut self) {
        self.retire_database_options_inspection_publication();
        self.retire_field_addition_inspection_publications();
        self.retire_field_rename_inspection();
        self.retire_field_drop_inspection();
        self.retire_table_drop_inspection();
        while let Some((generation, result)) = self.worker.poll() {
            if generation != self.generation {
                if self.owns_table_drop_inspection(generation) {
                    self.finish_superseded_table_drop_inspection(generation, result);
                    continue;
                }
                if self.state.pending_field_drop_inspection_id() == Some(generation) {
                    self.finish_superseded_field_drop_inspection(generation, result);
                    continue;
                }
                if self.state.pending_field_rename_inspection_id() == Some(generation) {
                    self.finish_superseded_field_rename_inspection(generation, result);
                    continue;
                }
                self.finish_superseded_field_addition_inspection(generation, result);
                continue;
            }
            self.state.busy = false;
            if let Err(error) = &result
                && self.recovery_open_failed(generation, error)
            {
                continue;
            }
            match result {
                Ok(Response::QuerySqlDraft { draft }) => {
                    self.publish_saved_query_sql(generation, draft);
                }
                Ok(Response::QuerySqlReviewed { review, capability }) => {
                    self.start_reviewed_query_definition(generation, review, capability);
                }
                Ok(Response::QueryExpressionReviewed { .. })
                | Ok(Response::AddColumnReviewed { .. })
                | Ok(Response::NumberColumnReviewed { .. })
                | Ok(Response::NumberColumnAvailability { .. }) => {
                    self.state.error = Some(self.state.localizer.text("ribbon-unavailable"));
                }
                Ok(Response::DatabaseOptionsInspected { inspection }) => {
                    self.publish_database_options_inspection(generation, inspection)
                }
                Ok(Response::DatabaseOptionsReviewed { review, capability }) => {
                    self.start_reviewed_database_options(generation, review, capability)
                }
                Ok(Response::TableDropSourceInspected { inspection }) => {
                    self.publish_table_drop_inspection(generation, inspection);
                }
                Ok(Response::TableDropReviewed { review, capability }) => {
                    if matches!(self.pending, PendingOperation::QueryActionReview { .. }) {
                        self.publish_query_schema_table_drop(generation, review, capability);
                    } else {
                        self.start_reviewed_table_drop(generation, review, capability);
                    }
                }
                Ok(Response::FieldDropSourceInspected { inspection }) => {
                    self.publish_field_drop_inspection(generation, inspection);
                }
                Ok(Response::FieldDropReviewed { review, capability }) => {
                    if matches!(self.pending, PendingOperation::QueryActionReview { .. }) {
                        self.publish_query_schema_field_drop(generation, review, capability);
                    } else {
                        self.start_reviewed_field_drop(generation, review, capability);
                    }
                }
                Ok(Response::FieldRenameSourceInspected { inspection }) => {
                    self.publish_field_rename_inspection(generation, inspection);
                }
                Ok(Response::FieldRenameReviewed { review, capability }) => {
                    self.start_reviewed_field_rename(generation, review, capability);
                }
                Ok(Response::FieldAdditionReviewed { review, capability }) => {
                    self.start_reviewed_field_addition(generation, review, capability);
                }
                Ok(Response::FieldAdditionAvailability { availability }) => {
                    self.publish_field_addition_availability(generation, availability);
                }
                Ok(Response::TableRenameReviewed { review, capability }) => {
                    self.start_reviewed_table_rename(generation, review, capability);
                }
                Ok(Response::TableCreationReviewed { capability }) => {
                    if matches!(self.pending, PendingOperation::QueryActionReview { .. }) {
                        self.publish_query_schema_creation(generation, capability);
                    } else {
                        self.start_reviewed_table_creation(generation, capability);
                    }
                }
                Ok(Response::QueryDesignTable { target, metadata }) => {
                    self.publish_query_design_table(generation, target, metadata);
                }
                Ok(Response::QueryDesignBuilt {
                    target,
                    built,
                    metadata,
                }) => self.publish_query_design_built(generation, target, built, metadata),
                Ok(Response::TotalsDescription(description)) => {
                    self.publish_totals_description(generation, description);
                }
                Ok(Response::Totals(result)) => self.publish_totals(generation, result),
                Ok(Response::FilteredData(result)) => self.publish_filtered(generation, result),
                Ok(Response::FieldFilterDescription(description)) => {
                    self.publish_field_filter_description(generation, description);
                }
                Ok(Response::Found { request, position }) => {
                    self.publish_found(generation, request, position);
                }
                Ok(Response::SavedQuery { target, parameters }) => {
                    self.publish_saved_query(generation, target, parameters);
                }
                Ok(Response::QueryParametersDiscovered { target, parameters }) => {
                    self.publish_discovered_query_parameters(generation, target, parameters);
                }
                Ok(Response::QueryParameters { target, parameters }) => {
                    self.publish_query_parameters(generation, target, parameters);
                }
                Ok(Response::QueryUpdateReviewed { review, capability }) => {
                    self.publish_query_update_review(generation, review, capability);
                }
                Ok(Response::QueryDeleteReviewed { review, capability }) => {
                    self.publish_query_delete_review(generation, review, capability);
                }
                Ok(Response::QueryInsertReviewed { review, capability }) => {
                    self.publish_query_insert_review(generation, review, capability);
                }
                Ok(Response::DeleteReviewed { review, capability }) => {
                    self.publish_delete_review(generation, review, capability);
                }
                Ok(Response::InsertDraft(draft)) => self.publish_insert_draft(generation, draft),
                Ok(Response::InsertReviewed { review, capability }) => {
                    self.publish_insert_review(generation, review, capability);
                }
                Ok(Response::EditDraft(draft)) => {
                    if matches!(self.pending, PendingOperation::ReplaceFind { .. }) {
                        self.publish_replace_find_draft(generation, draft);
                    } else {
                        self.publish_edit_draft(generation, draft);
                    }
                }
                Ok(Response::EditReviewed { review, capability }) => {
                    self.publish_edit_review(generation, review, capability);
                }
                Ok(Response::Details {
                    table,
                    declared_rows,
                    fields,
                }) => {
                    self.state.set_schema(Schema {
                        table,
                        declared_rows,
                        fields: crate::view_models::fields(fields),
                    });
                    self.inspect_field_addition_availability();
                }
                Ok(Response::Opened {
                    document_revision,
                    source_path,
                    table_creation_available,
                    table_design_available,
                    title,
                    format,
                    tables,
                    queries,
                }) => {
                    // NO_CLONE: Database table names move into the published catalog; the first
                    // selected table also needs an owned name for the separate explicit
                    // initial Open operation.
                    let first_table = tables
                        .iter()
                        .find(|table| !table.system && table.local)
                        .map(|table| table.name.clone());
                    let database = Database {
                        document_revision,
                        title: opening::document_title(&source_path, &title, &format),
                        title_path: Some(source_path.to_string_lossy().into_owned()),
                        format,
                        queries: queries
                            .into_iter()
                            .map(|query| SavedQuery {
                                object_id: query.object_id,
                                name: query.name,
                                system: query.system,
                            })
                            .collect(),
                        tables: tables
                            .into_iter()
                            .map(|table| Table {
                                name: table.name,
                                local: table.local,
                                system: table.system,
                                storage: table.storage,
                            })
                            .collect(),
                    };
                    if let PendingOperation::QuerySchemaReopen { request_id } = self.pending {
                        self.finish_query_schema_reopen(
                            request_id,
                            database,
                            table_creation_available,
                            table_design_available,
                        );
                        continue;
                    }
                    if let PendingOperation::DatabaseOptionsReopen { request_id } = self.pending {
                        self.finish_database_options_reopen(
                            request_id,
                            database,
                            table_creation_available,
                            table_design_available,
                        );
                        continue;
                    }
                    if let PendingOperation::TableDropReopen { request_id } = self.pending {
                        self.finish_table_drop_reopen(
                            request_id,
                            database,
                            table_creation_available,
                            table_design_available,
                        );
                        continue;
                    }
                    if let PendingOperation::FieldDropReopen { request_id } = self.pending {
                        self.finish_field_drop_reopen(
                            request_id,
                            database,
                            table_creation_available,
                            table_design_available,
                        );
                        continue;
                    }
                    if let PendingOperation::FieldRenameReopen { request_id } = self.pending {
                        self.finish_field_rename_reopen(
                            request_id,
                            database,
                            table_creation_available,
                            table_design_available,
                        );
                        continue;
                    }
                    if let PendingOperation::FieldAdditionReopen { request_id } = self.pending {
                        self.finish_field_addition_reopen(
                            request_id,
                            database,
                            table_creation_available,
                            table_design_available,
                        );
                        continue;
                    }
                    if let PendingOperation::QueryDefinitionReopen { request_id } = self.pending {
                        self.finish_query_definition_reopen(
                            request_id,
                            database,
                            table_creation_available,
                            table_design_available,
                        );
                        continue;
                    }
                    if let PendingOperation::RenamedTableReopen { request_id } = self.pending {
                        self.finish_table_rename_reopen(
                            request_id,
                            database,
                            table_creation_available,
                            table_design_available,
                        );
                        continue;
                    }
                    if let PendingOperation::CreatedTableReopen { request_id } = self.pending {
                        self.finish_table_creation_reopen(
                            request_id,
                            database,
                            table_creation_available,
                            table_design_available,
                        );
                        continue;
                    }
                    if let PendingOperation::InsertRetry {
                        request_id,
                        save_id,
                    } = self.pending
                    {
                        self.finish_insert_retry(
                            request_id,
                            save_id,
                            database,
                            table_creation_available,
                            table_design_available,
                        );
                        continue;
                    }
                    self.state.database = Some(database);
                    self.state
                        .set_table_creation_available(document_revision, table_creation_available);
                    self.state
                        .set_table_design_available(document_revision, table_design_available);
                    self.recovery_opened(generation, document_revision);
                    self.state.dismiss_backstage();
                    if matches!(self.pending, PendingOperation::CreatedOpen) {
                        self.state.open_initial_table_draft();
                    } else if !self.refresh_after_save(generation)
                        && let Some(table) = first_table
                    {
                        self.open_object(ObjectTarget {
                            document_revision,
                            kind: ObjectKind::Table { name: table },
                        });
                    }
                }
                Ok(Response::Data {
                    query_contents,
                    source_revision,
                    table,
                    offset,
                    columns,
                    rows,
                    has_more,
                }) => {
                    let query_owner = if matches!(
                        self.pending,
                        PendingOperation::Query { .. }
                            | PendingOperation::SavedRun { .. }
                            | PendingOperation::QueryDesignRun { .. }
                    ) {
                        self.state.active_object()
                    } else {
                        None
                    };
                    let bindings =
                        match self.query_contents_bindings(generation, query_contents.as_ref()) {
                            Ok(bindings) => bindings,
                            Err(()) => {
                                self.state.error =
                                    Some(self.state.localizer.text("objects-memory-limit"));
                                continue;
                            }
                        };
                    let data = Data {
                        query_contents: bindings,
                        row_ordinals: None,
                        source_revision,
                        table,
                        offset,
                        columns: crate::view_models::columns(columns),
                        rows: crate::view_models::rows(rows),
                        has_more,
                    };
                    match &self.pending {
                        PendingOperation::QueryDesignRun { request_id } => {
                            if !self.state.set_query_design_results(*request_id, data) {
                                self.state.fail_query_design(
                                    *request_id,
                                    self.state.localizer.text("qbe-invalid-result"),
                                );
                            }
                        }
                        PendingOperation::Query { request_id } => {
                            self.state.set_query_run_data(*request_id, data);
                        }
                        PendingOperation::SavedRun { target, request_id } => {
                            self.state.set_saved_query_data(*request_id, target, data);
                        }
                        PendingOperation::Record { target, request_id } => {
                            self.state.set_record_data(*request_id, target, data);
                        }
                        PendingOperation::FindPage {
                            request_id,
                            request,
                            position,
                            filter,
                        } => {
                            if filter.is_none() {
                                // NO_CLONE: The shared Find publisher retains found_request and the
                                // host retains its pending witness; this is an actual
                                // separate retained request, not only a comparison. The
                                // shared Find state retains the chosen match while the
                                // host keeps its FindPage witness; the owned column/span
                                // identity belongs to both states.
                                self.state.set_find_result(
                                    *request_id,
                                    request.clone(),
                                    Some(position.clone()),
                                    Some(data),
                                );
                            } else {
                                self.state.fail_find(
                                    *request_id,
                                    self.state.localizer.text("filtering-invalid-result"),
                                );
                            }
                        }
                        _ => {
                            if self.state.set_table_data(data) {
                                self.restore_saved_cell_selection(generation);
                                self.filter = None;
                                self.inspect_field_addition_availability();
                            }
                        }
                    }
                    if let Some(tab) = query_owner {
                        self.retain_query_contents(tab, generation, query_contents);
                    }
                }
                Ok(Response::QueryContents {
                    binding,
                    columns,
                    rows,
                }) => {
                    self.publish_query_contents(generation, binding, columns, rows);
                }
                Ok(Response::Contents {
                    target,
                    kind,
                    assigned,
                    columns,
                    rows,
                }) => {
                    if let PendingOperation::QueryActionReview { request_id } = &self.pending {
                        self.cancel_query_action_review(*request_id);
                        continue;
                    }
                    if let PendingOperation::Contents { request_id, .. } = &self.pending {
                        let target = ContentsTarget {
                            document_revision: target.document_revision,
                            table: target.table,
                            column: target.column,
                            parent_ordinal: target.parent_ordinal,
                        };
                        let data = Data {
                            query_contents: None,
                            row_ordinals: None,
                            // NO_CLONE: Collection Data.table display label and the ContentsTarget
                            // field identity are distinct owned DTO members; target must
                            // survive through guarded publication.
                            table: target.column.clone(),
                            source_revision: None,
                            offset: 0,
                            columns: crate::view_models::columns(columns),
                            rows: crate::view_models::rows(rows),
                            has_more: false,
                        };
                        self.state.set_contents(
                            *request_id,
                            &target,
                            crate::view_models::collection_kind(kind),
                            assigned,
                            data,
                        );
                    }
                }
                Err(error) => self.fail_read(generation, &error),
            }
        }
    }

    fn close_contents(&mut self, request_id: u64) {
        if matches!(&self.pending, PendingOperation::Contents { request_id: current, .. } | PendingOperation::QueryContents { request_id: current, .. } if *current == request_id)
            && self.generation == request_id
        {
            self.state.close_contents();
            if self.state.busy {
                self.worker.cancel();
                self.state.busy = false;
            }
            self.pending = PendingOperation::Read;
        }
    }
}

impl eframe::App for Desktop {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.query_contents.prune(&self.state);
        if let Some(families) = self.font_inventory.poll(ctx) {
            self.state.font_families = families;
        }
        self.release_stale_query_parameters();
        // A hidden startup frame can run logic without painting. Request the
        // initial window once, leaving later user hide/minimize choices alone.
        if std::mem::take(&mut self.startup_visibility_pending) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        }

        // eframe also calls logic for repaint requests while the viewport is
        // hidden. Task completion must not depend on drawing a visible frame.
        self.poll_updates(ctx);
        self.poll_file_dialog();
        self.poll_creation();
        self.poll();
        self.poll_saves();
        self.poll_recovery();
        if !self.state.insert_continuation_pending()
            && !self.state.insertion_dialog_open()
            && !self.editing.has_save()
        {
            self.editing.clear_insert_retry();
        }
        if !self.state.insert_continuation_pending()
            && !self.state.insertion_dialog_open()
            && !self.editing.has_save()
            && !self.state.busy
        {
            if let Some(target) = self.close_after_first_record.take()
                && let Some(tab) = self
                    .state
                    .object_tabs()
                    .into_iter()
                    .find(|tab| tab.target == target)
            {
                self.close_object(tab);
            }
            let table_transition = std::mem::take(&mut self.resume_after_table_creation);
            let query_transition = std::mem::take(&mut self.resume_after_query_definition);
            let field_transition = std::mem::take(&mut self.resume_after_field_schema);
            if table_transition || query_transition || field_transition {
                self.resume_table_draft_action(ctx);
            }
        }
        self.guard_save_close(ctx);
        self.guard_creation_close(ctx);
        self.guard_table_draft_window_close(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let locked = self.updates.installing() || self.update_exit_requested;
        let actions = ui.add_enabled_ui(!locked, |ui| self.state.show(ui)).inner;
        self.release_stale_query_parameters();
        // A manual edit ends the native selection binding, even if the user
        // later types the same lossy display string again.
        if self
            .selected_file
            .as_ref()
            .is_some_and(|selected| selected.path_for(&self.state.path).is_none())
        {
            self.selected_file = None;
        }
        self.invalidate_creation_selection();
        for action in actions {
            if self.updates.installing() || self.update_exit_requested {
                continue;
            }
            if self.recovery.is_some() && !editing::recovery::is_recovery_action(&action) {
                continue;
            }
            match action {
                Action::CheckForUpdates => self.start_update(UpdateCommand::Check),
                Action::DownloadUpdate => self.start_update(UpdateCommand::Download),
                Action::InstallUpdate => self.start_update(UpdateCommand::Install),
                Action::CancelUpdate => self.updates.cancel(),
                Action::BeginRecovery { attempt_id } => self.begin_reserved_recovery(attempt_id),
                Action::CloseRecovery { attempt_id } => self.close_reserved_recovery(attempt_id),
                Action::CancelRecovery { operation_id } => {
                    self.cancel_reserved_recovery(operation_id)
                }
                Action::ResolveRecovery {
                    operation_id,
                    recover,
                } => self.resolve_reserved_recovery(operation_id, recover),
                Action::AcknowledgeRecovery { operation_id } => {
                    self.acknowledge_reserved_recovery(operation_id)
                }
                Action::RenameTable { request } => self.rename_table(request),
                Action::CancelTableRenameReview { request_id } => {
                    self.cancel_table_rename_review(request_id)
                }
                Action::SaveTableDraft { request } => self.save_table_creation(request),
                Action::CancelTableDraftSave { id } => self.cancel_table_creation_save(id),
                Action::RecoverTableDraftSave { id } => self.resolve_table_creation_save(id, true),
                Action::ReleaseTableDraftSave { id } => self.resolve_table_creation_save(id, false),
                Action::AcknowledgeTableDraftSave { id } => {
                    self.acknowledge_table_creation_save(id)
                }
                Action::TableDraftsDiscarded => self.resume_table_draft_action(ui.ctx()),
                Action::TableDraftDiscardCancelled => self.cancel_table_draft_action(),
                Action::DescribeQueryDesign { request, table } => {
                    self.describe_query_design(request, table);
                }
                Action::BuildQueryDesign {
                    request,
                    draft,
                    completion,
                } => self.build_query_design(request, draft, completion),
                Action::ImportQueryDesign { request, sql } => {
                    self.import_query_design(request, sql);
                }
                Action::CancelQueryDesign { request_id } => self.cancel_query_design(request_id),
                Action::DescribeTotals { target } => self.describe_totals(target),
                Action::ComputeTotals { target, specs } => self.compute_totals(target, specs),
                Action::CancelTotals { request_id } => self.cancel_totals(request_id),
                Action::ChooseFile => {
                    if !self.creating.is_busy()
                        && !self.editing.has_save()
                        && !self.file_dialog.is_open()
                    {
                        self.file_dialog_purpose = FileDialogPurpose::Open;
                        self.file_dialog
                            .open(frame, ui.ctx(), &self.state.localizer);
                    }
                }
                Action::ChooseCreationLocation => {
                    if self.state.creation_available
                        && !self.state.busy
                        && !self.creating.is_busy()
                        && !self.editing.has_save()
                        && !self.file_dialog.is_open()
                    {
                        self.file_dialog_purpose = FileDialogPurpose::Creation;
                        self.file_dialog.save_database(
                            frame,
                            ui.ctx(),
                            &self.state.localizer,
                            self.state.creation_destination(),
                        );
                    }
                }
                Action::CreateDatabase { request } => self.create_database(request),
                Action::Open => self.open(),
                Action::Page { table, offset } => {
                    if let Some(target) = self
                        .state
                        .active_filter_target()
                        .filter(|target| target.table == table)
                    {
                        self.filter_page(target, offset, self.state.table_filter_enabled());
                    } else {
                        self.submit(Command::Page { table, offset });
                    }
                }
                Action::DescribeFieldFilter { target } => self.describe_field_filter(target),
                Action::ApplyFieldFilter {
                    target,
                    operator,
                    inputs,
                } => {
                    self.apply_field_filter(target, operator, inputs);
                }
                Action::CloseFieldFilter { request_id } => self.close_field_filter(request_id),
                Action::FilterSelection { target, comparison } => {
                    self.filter_selection(target, comparison)
                }
                Action::FilterPage {
                    target,
                    offset,
                    enabled,
                } => self.filter_page(target, offset, enabled),
                Action::ClearTableFilter { target } => self.clear_table_filter(target),
                Action::SortView {
                    target,
                    sort,
                    enabled,
                } => {
                    self.sort_view(target, sort, enabled);
                }
                Action::OpenObject { target } => self.open_object(target),
                Action::RetryObject { tab } => self.retry_object(tab),
                Action::ActivateObject { tab } => self.activate_object(tab),
                Action::CloseObject { tab } => self.close_object(tab),
                Action::ViewObjectDatasheet { tab } => self.view_object_datasheet(tab),
                Action::NavigateRecord { target } => self.navigate_record(target),
                Action::FindNext { request } => self.find_next(request),
                Action::ReplaceFindMatch { request } => self.replace_find_match(request),
                Action::CancelFind { request_id } => self.cancel_find(request_id),
                Action::ViewSchema { table } => self.view_object_schema(table),
                Action::ViewQueryContents { target } => self.view_query_contents(target),
                Action::ViewContents { target } => {
                    self.submit(Command::InspectCollection {
                        target: RuntimeContentsTarget {
                            document_revision: target.document_revision,
                            table: target.table,
                            column: target.column,
                            parent_ordinal: target.parent_ordinal,
                        },
                    });
                }
                Action::CloseContents { request_id } => self.close_contents(request_id),
                Action::InspectSavedQuery { target } => self.inspect_saved_query(target),
                Action::SavedQuerySqlClose { result } => {
                    self.finish_saved_query_sql_close(result, ui.ctx());
                }
                Action::FieldAdditionClose { result } => {
                    self.finish_field_addition_close(result, ui.ctx())
                }
                Action::InspectDatabaseOptions { document_revision } => {
                    self.inspect_database_options(document_revision)
                }
                Action::DisableNameAutoCorrect { request } => {
                    self.disable_name_auto_correct(request)
                }
                Action::CancelDatabaseOptionsReview { request_id } => {
                    self.cancel_database_options_review(request_id)
                }
                Action::DatabaseOptionsSave { identity, command } => {
                    self.database_options_save_command(identity, command)
                }
                Action::InspectTableDrop { request } => self.inspect_table_drop_source(request),
                Action::DeleteTable { request } => self.save_table_drop(request),
                Action::CancelTableDropInspection { request_id } => {
                    self.cancel_table_drop_inspection(request_id)
                }
                Action::CancelTableDropReview { request_id } => {
                    self.cancel_table_drop_review(request_id)
                }
                Action::TableDropSave { identity, command } => {
                    self.table_drop_save_command(identity, command)
                }
                Action::InspectFieldDrop { target } => self.inspect_field_drop_source(target),
                Action::ReviewFieldDrop { request } => self.save_field_drop(request),
                Action::CancelFieldDropInspection { request_id } => {
                    self.cancel_field_drop_inspection(request_id)
                }
                Action::CancelFieldDropReview { request_id } => {
                    self.cancel_field_drop_review(request_id)
                }
                Action::FieldDropSave { identity, command } => {
                    self.field_drop_save_command(identity, command)
                }
                Action::FieldDropClose { result } => self.finish_field_drop_close(result, ui.ctx()),
                Action::InspectFieldRename { target } => self.inspect_field_rename_source(target),
                Action::ReviewFieldRename { request } => self.save_field_rename(request),
                Action::CancelFieldRenameReview { request_id } => {
                    self.cancel_field_rename_review(request_id)
                }
                Action::FieldRenameSave { identity, command } => {
                    self.field_rename_save_command(identity, command)
                }
                Action::FieldRenameClose { result } => {
                    self.finish_field_rename_close(result, ui.ctx())
                }
                Action::CloseFieldRenameEditor => self.close_field_rename_editor(),
                Action::ReviewFieldAddition { request } => self.save_field_addition(request),
                Action::RetryFieldAdditionInspection { tab } => {
                    self.retry_field_addition_availability(tab)
                }
                Action::CancelFieldAdditionReview { request_id } => {
                    self.cancel_field_addition_review(request_id)
                }
                Action::FieldAdditionSave { identity, command } => {
                    self.field_addition_save_command(identity, command)
                }
                Action::ReviewSavedQuerySql { request } => {
                    self.save_query_definition(request);
                }
                Action::SavedQuerySqlSave { identity, command } => {
                    self.query_definition_save_action(identity, command);
                }
                Action::RunSavedQuery { target, parameters } => {
                    self.run_saved_query(target, parameters);
                }
                Action::RetryQuerySchemaReopen { document_revision } => {
                    self.retry_query_schema_reopen(document_revision);
                }
                Action::ApplyQueryAction { review_id } => self.apply_query_action(review_id),
                Action::CancelQueryActionReview { request_id } => {
                    self.cancel_query_action_review(request_id);
                }
                Action::ConfigureQueryRunLimits { limits } => {
                    self.configure_query_run_limits(limits);
                }
                Action::Query => self.inspect_query_parameters(),
                Action::RunParameterizedQuery {
                    inspection_id,
                    parameters,
                } => {
                    self.run_parameterized_query(inspection_id, parameters);
                }
                Action::ReviewDelete { target } => self.review_delete(target),
                Action::ApplyDelete { review_id } => self.apply_delete(review_id),
                Action::CloseDelete { request_id } => self.close_delete(request_id),
                Action::BeginInsert { target } => self.begin_insert(target),
                Action::RetryInsert { save_id } => self.retry_insert(save_id),
                Action::DiscardRetainedInsert { save_id } => self.discard_insert_retry(save_id),
                Action::CancelInsertRetry { operation_id } => {
                    self.cancel_insert_retry(operation_id)
                }
                Action::ReviewInsert { draft_id, inputs } => self.review_insert(draft_id, inputs),
                Action::ApplyInsert { review_id } => self.apply_insert(review_id),
                Action::CloseInsert { request_id, .. } => self.close_insert(request_id),
                Action::BeginEdit { target } => self.begin_edit(target),
                Action::BeginSeededEdit { target, seed } => self.begin_seeded_edit(target, seed),
                Action::PasteCell { target, text } => self.begin_paste_edit(target, text),
                Action::ReviewEdit { draft_id, input } => self.review_edit(draft_id, input),
                Action::ApplyEdit { review_id } => self.apply_edit(review_id),
                Action::CloseEdit { request_id, .. } => self.close_edit(request_id),
                Action::CancelEditSave { save_id } => self.cancel_save(save_id),
                Action::RecoverEditSave { save_id } => self.resolve_save(save_id, true),
                Action::ReleaseEditSave { save_id } => self.resolve_save(save_id, false),
                Action::AcknowledgeEditSave { save_id } => self.acknowledge_save(save_id),
                Action::Cancel => {
                    if self.table_creation.is_some() {
                        self.cancel_active_table_creation_save();
                        continue;
                    }
                    if self.creating.is_busy() {
                        self.cancel_creation();
                        continue;
                    }
                    if self.editing.has_save() {
                        self.cancel_current_save();
                        continue;
                    }
                    if let PendingOperation::Contents { request_id, .. }
                    | PendingOperation::QueryContents { request_id, .. } = &self.pending
                    {
                        self.close_contents(*request_id);
                    } else {
                        self.worker.cancel();
                        self.state.busy = false;
                        self.state.clear_query_run();
                        self.clear_query_parameters();
                        match &self.pending {
                            PendingOperation::TableDropInspection { request_id } => {
                                self.state.interrupt_table_drop_inspection(*request_id);
                                self.pending = PendingOperation::Read;
                            }
                            PendingOperation::FieldDropInspection { request_id } => {
                                self.state.interrupt_field_drop_inspection_id(*request_id);
                                self.pending = PendingOperation::Read;
                            }
                            PendingOperation::FieldRenameInspection { request_id } => {
                                self.state.interrupt_field_rename_inspection_id(*request_id);
                                self.pending = PendingOperation::Read;
                            }
                            PendingOperation::FieldAdditionAvailability { request_id, .. } => {
                                // This exact readonly publication was revoked.
                                // Keep its field-level interruption/retry state,
                                // without creating an unrelated global error.
                                self.state.interrupt_field_addition_inspection(*request_id);
                                self.state.prune_field_addition_inspections();
                                self.pending = PendingOperation::Read;
                            }
                            PendingOperation::ReplaceFind { request_id } => {
                                self.state.fail_replace_find(
                                    *request_id,
                                    self.state.localizer.text("shell-operation-cancelled"),
                                );
                            }
                            PendingOperation::QueryDesignDescribe { request_id, .. }
                            | PendingOperation::QueryDesignBuild { request_id, .. }
                            | PendingOperation::QueryDesignImport { request_id, .. }
                            | PendingOperation::QueryDesignRun { request_id } => {
                                self.state.fail_query_design(
                                    *request_id,
                                    self.state.localizer.text("shell-operation-cancelled"),
                                );
                            }
                            PendingOperation::TotalsDescribe { request_id, .. }
                            | PendingOperation::TotalsCompute { request_id, .. } => {
                                self.state.fail_totals(
                                    *request_id,
                                    self.state.localizer.text("shell-operation-cancelled"),
                                );
                            }
                            PendingOperation::FieldFilterDescribe { request_id, .. }
                            | PendingOperation::FieldFilterApply { request_id } => {
                                self.state.fail_field_filter(
                                    *request_id,
                                    self.state.localizer.text("shell-operation-cancelled"),
                                );
                            }
                            PendingOperation::FilterSelection { request_id }
                            | PendingOperation::FilterPage { request_id }
                            | PendingOperation::FilterRestore { request_id }
                            | PendingOperation::SortView { request_id }
                            | PendingOperation::ClearFilter { request_id } => {
                                self.state.fail_filter(
                                    *request_id,
                                    self.state.localizer.text("shell-operation-cancelled"),
                                );
                            }
                            PendingOperation::Find { request_id, .. }
                            | PendingOperation::FindPage { request_id, .. } => {
                                self.state.fail_find(
                                    *request_id,
                                    self.state.localizer.text("shell-operation-cancelled"),
                                );
                            }
                            PendingOperation::Delete { request_id } => {
                                self.state.fail_delete(
                                    *request_id,
                                    self.state.localizer.text("shell-operation-cancelled"),
                                );
                            }
                            PendingOperation::Insert { request_id } => {
                                self.state.fail_insert(
                                    *request_id,
                                    self.state.localizer.text("shell-operation-cancelled"),
                                );
                            }
                            PendingOperation::Edit { request_id } => {
                                self.state.fail_edit(
                                    *request_id,
                                    self.state.localizer.text("shell-operation-cancelled"),
                                );
                            }
                            PendingOperation::Record { request_id, .. } => {
                                self.state.fail_record_navigation(
                                    *request_id,
                                    self.state.localizer.text("shell-operation-cancelled"),
                                );
                            }
                            PendingOperation::SavedSqlInspection {
                                request_id,
                                request,
                            } => {
                                self.state.fail_saved_query_sql(
                                    *request_id,
                                    request,
                                    self.state.localizer.text("shell-operation-cancelled"),
                                );
                            }
                            PendingOperation::SavedInspection { target, request_id }
                            | PendingOperation::SavedRun { target, request_id } => {
                                self.state.fail_saved_query(
                                    *request_id,
                                    target,
                                    self.state.localizer.text("shell-operation-cancelled"),
                                );
                            }
                            _ => {
                                self.state.error =
                                    Some(self.state.localizer.text("shell-operation-cancelled"))
                            }
                        }
                    }
                }
            }
        }
        // UI actions can acquire a new owner after logic checked this frame's
        // close event. Retain it without treating that earlier event as a new
        // cancellation request for an operation the user just accepted.
        if ui.input(|input| input.viewport().close_requested())
            && (self.editing.has_save()
                || self.creating.is_busy()
                || self.state.insertion_dialog_open()
                || self.state.insert_continuation_pending())
        {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        self.guard_table_draft_window_close(ui.ctx());
    }
}

impl Drop for Desktop {
    fn drop(&mut self) {
        self.state.password.zeroize();
        // Worker::drop revokes reads without taking GUI-thread mailbox locks.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::App;
    use std::path::Path;
    use std::time::{Duration, Instant};
    use tablecraft_testkit::fixtures::access::attachment_modern;

    fn finish(desktop: &mut Desktop, context: &egui::Context, frame: &mut eframe::Frame) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while desktop.state.busy && Instant::now() < deadline {
            desktop.logic(context, frame);
            std::thread::yield_now();
        }
        assert!(!desktop.state.busy, "desktop task did not finish");
    }

    fn open_sql(desktop: &mut Desktop) -> tablecraft_ui::ObjectTab {
        let target = tablecraft_ui::ObjectTarget {
            document_revision: desktop.state.database.as_ref().unwrap().document_revision,
            kind: tablecraft_ui::ObjectKind::Sql,
        };
        desktop.open_object(target);
        desktop.state.active_object().unwrap()
    }

    #[test]
    fn startup_logic_requests_visibility_once_without_revealing_later_hidden_frames() {
        let context = egui::Context::default();
        let mut desktop = Desktop::new(context.clone(), None);
        let mut frame = eframe::Frame::_new_kittest();
        let mut input = egui::RawInput::default();
        let viewport = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
        viewport.occluded = Some(true);
        viewport.minimized = Some(false);

        // Use the same logic-only entry point as eframe's hidden-window path;
        // no UI pass is needed to issue the initial visibility request.
        let initial = context.run_logic(&input, |ctx| desktop.logic(ctx, &mut frame));
        assert_eq!(
            initial.viewport_commands[&egui::ViewportId::ROOT],
            [egui::ViewportCommand::Visible(true)]
        );

        for minimized in [false, true, false] {
            input
                .viewports
                .get_mut(&egui::ViewportId::ROOT)
                .unwrap()
                .minimized = Some(minimized);
            let later = context.run_logic(&input, |ctx| desktop.logic(ctx, &mut frame));
            assert!(
                later
                    .viewport_commands
                    .values()
                    .all(|commands| !commands.contains(&egui::ViewportCommand::Visible(true)))
            );
        }
    }

    #[test]
    fn task_completion_is_consumed_without_drawing_a_visible_frame() {
        let context = egui::Context::default();
        let mut desktop = Desktop::new(context.clone(), None);
        let mut frame = eframe::Frame::_new_kittest();
        desktop.submit(Command::Page {
            table: "Items".into(),
            offset: 0,
        });
        finish(&mut desktop, &context, &mut frame);
        assert_eq!(desktop.state.error.as_deref(), Some("no database is open"));
    }

    #[test]
    fn backstage_stays_open_on_failure_and_closes_only_after_open_succeeds() {
        let directory = tempfile::tempdir().unwrap();
        let context = egui::Context::default();
        let mut desktop = Desktop::new(context.clone(), None);
        let mut frame = eframe::Frame::_new_kittest();
        assert!(desktop.state.backstage_visible());

        // A workspace picker selection must expose the Open surface immediately,
        // including when the asynchronous open subsequently fails.
        desktop.state.dismiss_backstage();
        desktop.state.password = "pending credential".into();
        desktop.file_selected(Some(directory.path().join("missing.accdb")));
        assert!(desktop.state.backstage_visible());
        assert!(desktop.state.busy);
        assert!(desktop.state.password.is_empty());
        finish(&mut desktop, &context, &mut frame);
        assert!(desktop.state.backstage_visible());
        assert!(desktop.state.database.is_none());
        assert!(desktop.state.error.is_some());

        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/examples/Access_Example_VBA.accdb");
        desktop.file_selected(Some(path));
        assert!(desktop.state.backstage_visible());
        assert!(desktop.state.busy);
        finish(&mut desktop, &context, &mut frame);
        assert!(desktop.state.database.is_some());
        assert!(desktop.state.error.is_none());
        assert!(!desktop.state.backstage_visible());

        // Entering Backstage does not cancel a task, and a query response does
        // not dismiss it as though another document had just opened.
        open_sql(&mut desktop);
        desktop.submit(Command::Query {
            options: Default::default(),
            sql: "SELECT 17 AS [Kept]".into(),
        });
        let generation = desktop.generation;
        desktop.state.show_open();
        assert_eq!(desktop.generation, generation);
        assert!(desktop.state.busy);
        finish(&mut desktop, &context, &mut frame);
        assert!(desktop.state.backstage_visible());
        assert_eq!(
            desktop.state.query_data.as_ref().unwrap().rows[0][0].text,
            "17"
        );
    }

    #[test]
    fn cancelling_the_picker_preserves_the_document_views_and_open_inputs() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/examples/Access_Example_VBA.accdb");
        let context = egui::Context::default();
        let mut desktop = Desktop::new(context.clone(), Some(path.clone()));
        let mut frame = eframe::Frame::_new_kittest();
        finish(&mut desktop, &context, &mut frame);
        let table_tab = desktop.state.active_object().unwrap();
        let table_rows = desktop.state.data.as_ref().unwrap().rows.as_ptr();
        let sql_tab = open_sql(&mut desktop);
        desktop.submit(Command::Query {
            options: Default::default(),
            sql: "SELECT 17 AS [Kept]".into(),
        });
        finish(&mut desktop, &context, &mut frame);
        let query_rows = desktop.state.query_data.as_ref().unwrap().rows.as_ptr();
        desktop.view_object_schema("tblContacts".into());
        finish(&mut desktop, &context, &mut frame);
        assert!(desktop.state.error.is_none());

        desktop.state.show_open();
        desktop.state.password = "unsubmitted credential".into();
        desktop.state.error = Some("retained diagnostic".into());
        let generation = desktop.generation;
        let revision = desktop.state.database.as_ref().unwrap().document_revision;
        let active_tab = desktop.state.active_object().unwrap();
        let tabs = desktop.state.object_tabs();
        let fields = desktop.state.schema.as_ref().unwrap().fields.as_ptr();

        desktop.file_selected(None);
        assert_eq!(desktop.generation, generation);
        assert!(!desktop.state.busy);
        assert!(desktop.state.backstage_visible());
        assert_eq!(desktop.state.password, "unsubmitted credential");
        assert_eq!(desktop.state.error.as_deref(), Some("retained diagnostic"));
        assert_eq!(
            desktop.state.database.as_ref().unwrap().document_revision,
            revision
        );
        assert_eq!(desktop.state.active_object(), Some(active_tab));
        assert_eq!(desktop.state.object_tabs(), tabs);
        assert_eq!(
            desktop.state.schema.as_ref().unwrap().fields.as_ptr(),
            fields
        );
        assert_eq!(
            desktop
                .selected_file
                .as_ref()
                .unwrap()
                .path_for(&desktop.state.path),
            Some(path.as_path())
        );

        desktop.state.dismiss_backstage();
        desktop.activate_object(sql_tab);
        assert_eq!(
            desktop.state.query_data.as_ref().unwrap().rows.as_ptr(),
            query_rows
        );
        desktop.activate_object(table_tab.clone());
        desktop.view_object_datasheet(table_tab);
        finish(&mut desktop, &context, &mut frame);
        assert_eq!(
            desktop.state.data.as_ref().unwrap().rows.as_ptr(),
            table_rows
        );
        desktop.file_selected(None);
        assert!(!desktop.state.backstage_visible());
        assert_eq!(desktop.generation, generation);
    }

    #[test]
    fn schema_response_is_adapted_and_cleared_when_the_document_changes() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/examples/Access_Example_VBA.accdb");
        let context = egui::Context::default();
        let mut desktop = Desktop::new(context.clone(), Some(path));
        let mut frame = eframe::Frame::_new_kittest();
        finish(&mut desktop, &context, &mut frame);
        assert!(desktop.state.database.is_some());

        let original_tab = desktop.state.active_object().unwrap();
        let original_rows = desktop.state.data.as_ref().unwrap().rows.as_ptr();
        desktop.view_object_schema("tblContacts".to_owned());
        finish(&mut desktop, &context, &mut frame);
        let schema = desktop.state.schema.as_ref().expect("schema response");
        assert_eq!(schema.table, "tblContacts");
        assert_eq!(schema.fields[0].name, "ID");
        assert_eq!(schema.fields[0].scalar_type.as_deref(), Some("Int32"));
        let contacts_tab = desktop.state.active_object().unwrap();
        assert_eq!(
            contacts_tab.target.kind,
            tablecraft_ui::ObjectKind::Table {
                name: "tblContacts".into(),
            }
        );
        assert_ne!(contacts_tab, original_tab);
        assert!(desktop.state.object_tabs().contains(&original_tab));
        desktop.view_object_datasheet(contacts_tab.clone());
        finish(&mut desktop, &context, &mut frame);
        assert_eq!(desktop.state.active_object(), Some(contacts_tab.clone()));
        assert_eq!(desktop.state.data.as_ref().unwrap().table, "tblContacts");
        desktop.activate_object(original_tab.clone());
        assert_eq!(desktop.state.active_object(), Some(original_tab));
        assert_eq!(
            desktop.state.data.as_ref().unwrap().rows.as_ptr(),
            original_rows
        );
        desktop.activate_object(contacts_tab);
        assert_eq!(desktop.state.schema.as_ref().unwrap().table, "tblContacts");

        // Reopening clears the old schema before the next request completes.
        desktop.submit(Command::Describe {
            table: "tblContacts".to_owned(),
        });
        desktop.open();
        assert!(desktop.state.schema.is_none());
        finish(&mut desktop, &context, &mut frame);
        assert!(desktop.state.database.is_some());
        assert!(desktop.state.schema.is_none());
    }

    #[test]
    fn query_null_and_literal_null_keep_distinct_cell_facts_through_the_host() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/examples/Access_Example_VBA.accdb");
        let context = egui::Context::default();
        let mut desktop = Desktop::new(context.clone(), Some(path));
        let mut frame = eframe::Frame::_new_kittest();
        finish(&mut desktop, &context, &mut frame);
        let table_tab = desktop.state.active_object().unwrap();
        let table_rows = desktop.state.data.as_ref().unwrap().rows.as_ptr();
        let sql_tab = open_sql(&mut desktop);
        desktop.submit(Command::Query {
            options: Default::default(),
            sql: "SELECT NULL AS [Absent], 'NULL' AS [Label], True AS [Yes], False AS [No], 'false' AS [Text]".into(),
        });
        finish(&mut desktop, &context, &mut frame);

        let data = desktop.state.query_data.as_ref().expect("query result");
        assert_eq!(data.rows[0][0].text, "NULL");
        assert_eq!(data.rows[0][1].text, "NULL");
        assert!(matches!(
            data.rows[0][0].kind,
            tablecraft_ui::CellKind::Scalar {
                is_null: true,
                stored_calculation: false,
                boolean: None,
                display_control: tablecraft_ui::DisplayControl::Uninspected
            }
        ));
        assert!(matches!(
            data.rows[0][1].kind,
            tablecraft_ui::CellKind::Scalar {
                is_null: false,
                stored_calculation: false,
                boolean: None,
                display_control: tablecraft_ui::DisplayControl::Uninspected
            }
        ));
        for (index, boolean) in [(2, Some(true)), (3, Some(false)), (4, None)] {
            assert!(matches!(
                data.rows[0][index].kind,
                tablecraft_ui::CellKind::Scalar {
                    is_null: false,
                    stored_calculation: false,
                    boolean: actual,
                display_control: tablecraft_ui::DisplayControl::Uninspected
                } if actual == boolean
            ));
        }
        assert_eq!(data.rows[0][3].text, data.rows[0][4].text);
        desktop.activate_object(table_tab);
        assert_eq!(
            desktop.state.data.as_ref().unwrap().rows.as_ptr(),
            table_rows
        );
        desktop.activate_object(sql_tab);
        assert_eq!(
            desktop.state.query_data.as_ref().unwrap().rows[0][0].text,
            "NULL"
        );
    }

    #[test]
    fn collection_metadata_roundtrip_keeps_source_and_dialog_requests_separate() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("attachments.accdb");
        let original = attachment_modern::database(6);
        std::fs::write(&path, &original).unwrap();
        let context = egui::Context::default();
        context.enable_accesskit();
        let mut desktop = Desktop::new(context.clone(), Some(path.clone()));
        let mut frame = eframe::Frame::_new_kittest();
        finish(&mut desktop, &context, &mut frame);
        let revision = desktop.state.database.as_ref().unwrap().document_revision;
        desktop.open_object(tablecraft_ui::ObjectTarget {
            document_revision: revision,
            kind: tablecraft_ui::ObjectKind::Table {
                name: "Items".into(),
            },
        });
        finish(&mut desktop, &context, &mut frame);

        let target = RuntimeContentsTarget {
            document_revision: revision,
            table: "Items".into(),
            column: "Score".into(),
            parent_ordinal: 0,
        };
        desktop.submit(Command::InspectCollection {
            target: target.clone(),
        });
        let first_request = desktop.generation;
        finish(&mut desktop, &context, &mut frame);
        assert_eq!(desktop.state.contents_request_id(), Some(first_request));
        assert_eq!(
            desktop.state.data.as_ref().unwrap().rows[0][1].text,
            "2 attachments"
        );

        let mut visible_metadata = false;
        for pass in 0..3 {
            let mut output = context.run_ui(
                egui::RawInput {
                    time: Some(f64::from(pass) * 0.25),
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(1280.0, 820.0),
                    )),
                    ..Default::default()
                },
                |ui| assert!(desktop.state.show(ui).is_empty()),
            );
            visible_metadata = output
                .platform_output
                .accesskit_update
                .as_ref()
                .is_some_and(|tree| {
                    tree.nodes.iter().any(|(_, node)| {
                        node.value()
                            .is_some_and(|text| text.contains("synthetic.bin"))
                    })
                });
            output.textures_delta.clear();
        }
        assert!(visible_metadata, "collection metadata was not presented");
        desktop.close_contents(first_request);
        assert!(desktop.state.contents_request_id().is_none());

        // An old dialog close must not cancel a newer request for the same cell.
        desktop.submit(Command::InspectCollection {
            target: target.clone(),
        });
        let second_request = desktop.generation;
        desktop.close_contents(first_request);
        assert_eq!(desktop.state.contents_request_id(), Some(second_request));
        assert!(desktop.state.busy);
        desktop.close_contents(second_request);
        assert!(desktop.state.contents_request_id().is_none());
        assert!(!desktop.state.busy);

        desktop.open();
        finish(&mut desktop, &context, &mut frame);
        assert_ne!(
            desktop.state.database.as_ref().unwrap().document_revision,
            revision
        );
        let before = desktop.generation;
        desktop.submit(Command::InspectCollection { target });
        assert_eq!(desktop.generation, before);
        assert!(desktop.state.contents_request_id().is_none());
        assert!(!desktop.state.busy);
        assert!(desktop.state.error.is_some());
        assert_eq!(std::fs::read(path).unwrap(), original);
    }
}
