mod active_save;
mod backstage;
mod chrome;
mod click;
mod content;
mod contents;
mod creation;
mod database_options;
mod deletion;
mod diagnostics;
mod editing;
pub(crate) mod field_addition;
mod field_name;
pub(crate) mod field_rename;
mod filtering;
mod find;
pub(crate) mod focus;
mod home;
pub(crate) mod insertion;
mod native_mutation;
mod navigation;
mod objects;
pub mod preview;
mod query;
mod query_design;
mod records;
mod recovery;
pub(crate) mod ribbon;
mod row_identity;
mod save_display;
mod saved_query;
mod saved_query_save;
mod schema;
mod schema_save;
mod table_draft;
#[cfg(test)]
mod test_support;
pub(crate) mod totals;
pub(crate) mod zoom;

pub use contents::{CollectionKind, ContentsTarget};
pub use creation::{CreationOutcome, CreationRequest};
pub use database_options::{
    DatabaseOptionsCloseRequest, DatabaseOptionsSaveCommand, DatabaseOptionsSaveIdentity,
    NameAutoCorrectState, PreparedDatabaseOptionsSave,
};
pub use deletion::{DeleteReview, DeleteTarget};
pub use editing::{EditDraft, EditInput, EditReview, EditSaveOutcome, EditTarget, InputHint};
pub use field_addition::{
    FieldAdditionChoices, FieldAdditionInspectionError, FieldAdditionRequest,
    FieldAdditionSaveCommand, FieldAdditionSaveIdentity, FieldAdditionSavedWarning,
    PreparedFieldAdditionSave,
};
pub use field_addition::{FieldAdditionCloseDecision, FieldAdditionCloseResult};
pub use field_rename::{
    FieldRenameCloseDecision, FieldRenameCloseResult, PreparedCleanFieldRenameClose,
};
pub use field_rename::{
    FieldRenameRequest, FieldRenameSaveCommand, FieldRenameSaveIdentity, FieldRenameTarget,
    FieldRenameWarning, PreparedFieldRenameInspection, PreparedFieldRenameResolution,
    PreparedFieldRenameSave, PreparedUnchangedFieldRename,
};
pub use filtering::{
    FieldFilterDescription, FieldFilterOperator, FieldFilterPatternMode, FieldFilterTarget,
    FilterComparison, FilterPublication, FilterRestore, FilterSelectionTarget, FilterTarget,
    SortSpec, SortTarget,
};
pub use find::{
    FindDirection, FindMatch, FindPosition, FindRequest, FindScope, FindSpan, FindTarget,
    ReplaceFindRequest,
};
pub use insertion::{
    InsertDraft, InsertField, InsertInput, InsertRetryPermission, InsertReview, InsertTarget,
};
pub use navigation::rename::TableRenameRequest;
pub use objects::{ObjectKind, ObjectOpen, ObjectTab, ObjectTarget, WorkspaceLayout};
pub use query::actions::review::{QueryActionEffect, QueryActionKind, QueryActionReview};
pub use query::parameters::{ParameterInput, ParameterSpec};
pub use query_design::{QueryDesignCompletion, QueryDesignRequest, QueryDesignTarget};
pub use records::RecordTarget;
pub use recovery::RecoveryOutcome;
pub use row_identity::{RowOrdinalError, RowOrdinals};
pub use save_display::SaveCleanupNotice;
pub use saved_query::{
    SavedQuery, SavedQuerySqlCloseDecision, SavedQuerySqlCloseResult, SavedQuerySqlEligibility,
    SavedQuerySqlError, SavedQuerySqlRequest, SavedQueryTarget,
};
pub use saved_query_save::{
    SavedQuerySqlSaveCommand, SavedQuerySqlSaveIdentity, SavedQuerySqlSaveOperation,
};
pub use schema::{Field, FieldProperty, FieldPropertyValue, Schema};
pub use table_draft::{InitialRecord, TableDraftSaveCompletion, TableSaveRequest};
pub use totals::{
    TotalAggregate, TotalSpec, TotalsData, TotalsDescription, TotalsField, TotalsTarget,
};

use super::{Cell, Column, keyboard};
use crate::datasheet::Selection;
use egui::Ui;
use query::provenance::{PendingSql, ResultOrigin};
use tablecraft_display::Options;

/// One inert catalog entry. A visible declaration does not imply readable rows.
pub struct Table {
    pub name: String,
    pub local: bool,
    pub system: bool,
    pub storage: String,
}

/// Opened database metadata supplied by the application host.
pub struct Database {
    /// Captured open-source path for title hover, independent of editable Open text.
    pub title_path: Option<String>,
    pub document_revision: u64,
    pub title: String,
    pub format: String,
    pub tables: Vec<Table>,
    pub queries: Vec<SavedQuery>,
}

/// One bounded buffered page or query result supplied by the host.
pub struct Data {
    pub source_revision: Option<u64>,
    /// Physical native membership for a sparse display page; offset stays visible.
    pub row_ordinals: Option<RowOrdinals>,
    pub table: String,
    pub offset: u32,
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Cell>>,
    pub has_more: bool,
}

/// User intents dispatched to the host's shared backend services.
pub enum Action {
    InspectFieldRename {
        target: FieldRenameTarget,
    },
    ReviewFieldRename {
        request: FieldRenameRequest,
    },
    CancelFieldRenameReview {
        request_id: u64,
    },
    FieldRenameSave {
        identity: FieldRenameSaveIdentity,
        command: FieldRenameSaveCommand,
    },
    FieldRenameClose {
        result: FieldRenameCloseResult,
    },
    CloseFieldRenameEditor,
    InspectDatabaseOptions {
        document_revision: u64,
    },
    DisableNameAutoCorrect {
        request: DatabaseOptionsCloseRequest,
    },
    CancelDatabaseOptionsReview {
        request_id: u64,
    },
    DatabaseOptionsSave {
        identity: DatabaseOptionsSaveIdentity,
        command: DatabaseOptionsSaveCommand,
    },
    FieldAdditionClose {
        result: FieldAdditionCloseResult,
    },
    ReviewFieldAddition {
        request: FieldAdditionRequest,
    },
    RetryFieldAdditionInspection {
        tab: ObjectTab,
    },
    CancelFieldAdditionReview {
        request_id: u64,
    },
    FieldAdditionSave {
        identity: FieldAdditionSaveIdentity,
        command: FieldAdditionSaveCommand,
    },
    /// Cancel an issued table-rename review before a save is accepted.
    CancelTableRenameReview {
        /// Exact naming request whose review should be cancelled.
        request_id: u64,
    },
    /// Review and save the user's explicit closed-table naming request.
    RenameTable {
        /// Original source revision and exact old/new catalog spellings.
        request: TableRenameRequest,
    },
    ApplyQueryAction {
        review_id: u64,
    },
    CancelQueryActionReview {
        request_id: u64,
    },
    BeginRecovery {
        attempt_id: u64,
    },
    CloseRecovery {
        attempt_id: u64,
    },
    CancelRecovery {
        operation_id: u64,
    },
    ResolveRecovery {
        operation_id: u64,
        recover: bool,
    },
    AcknowledgeRecovery {
        operation_id: u64,
    },
    SaveTableDraft {
        request: TableSaveRequest,
    },
    CancelTableDraftSave {
        id: u64,
    },
    RecoverTableDraftSave {
        id: u64,
    },
    ReleaseTableDraftSave {
        id: u64,
    },
    AcknowledgeTableDraftSave {
        id: u64,
    },
    SavedQuerySqlSave {
        identity: SavedQuerySqlSaveIdentity,
        command: SavedQuerySqlSaveCommand,
    },
    SavedQuerySqlClose {
        result: SavedQuerySqlCloseResult,
    },
    TableDraftsDiscarded,
    TableDraftDiscardCancelled,
    ChooseFile,
    ChooseCreationLocation,
    CreateDatabase {
        request: CreationRequest,
    },
    Open,
    Page {
        table: String,
        offset: u32,
    },
    ReviewSavedQuerySql {
        request: SavedQuerySqlRequest,
    },
    InspectSavedQuery {
        target: SavedQueryTarget,
    },
    RunParameterizedQuery {
        inspection_id: u64,
        parameters: Vec<ParameterInput>,
    },
    RunSavedQuery {
        target: SavedQueryTarget,
        parameters: Vec<ParameterInput>,
    },
    NavigateRecord {
        target: RecordTarget,
    },
    ViewSchema {
        table: String,
    },
    ViewContents {
        target: ContentsTarget,
    },
    CloseContents {
        request_id: u64,
    },
    OpenObject {
        target: ObjectTarget,
    },
    ActivateObject {
        tab: ObjectTab,
    },
    CloseObject {
        tab: ObjectTab,
    },
    RetryObject {
        tab: ObjectTab,
    },
    ViewObjectDatasheet {
        tab: ObjectTab,
    },
    SortView {
        target: SortTarget,
        sort: Option<SortSpec>,
        enabled: bool,
    },
    BeginEdit {
        target: EditTarget,
    },
    PasteCell {
        target: EditTarget,
        text: String,
    },
    DescribeTotals {
        target: TotalsTarget,
    },
    ComputeTotals {
        target: TotalsTarget,
        specs: Vec<TotalSpec>,
    },
    CancelTotals {
        request_id: u64,
    },
    DescribeFieldFilter {
        target: FieldFilterTarget,
    },
    ApplyFieldFilter {
        target: FieldFilterTarget,
        operator: FieldFilterOperator,
        inputs: Vec<String>,
    },
    CloseFieldFilter {
        request_id: u64,
    },
    FilterSelection {
        target: FilterSelectionTarget,
        comparison: FilterComparison,
    },
    FilterPage {
        target: FilterTarget,
        offset: u32,
        enabled: bool,
    },
    ClearTableFilter {
        target: FilterTarget,
    },
    ReplaceFindMatch {
        request: ReplaceFindRequest,
    },
    FindNext {
        request: FindRequest,
    },
    CancelFind {
        request_id: u64,
    },
    ReviewDelete {
        target: DeleteTarget,
    },
    ApplyDelete {
        review_id: u64,
    },
    CloseDelete {
        request_id: u64,
    },
    BeginInsert {
        target: InsertTarget,
    },
    ReviewInsert {
        draft_id: u64,
        inputs: Vec<InsertInput>,
    },
    ApplyInsert {
        review_id: u64,
    },
    CloseInsert {
        request_id: u64,
        draft_id: Option<u64>,
    },
    ReviewEdit {
        draft_id: u64,
        input: EditInput,
    },
    ApplyEdit {
        review_id: u64,
    },
    CloseEdit {
        request_id: u64,
        draft_id: Option<u64>,
    },
    CancelEditSave {
        save_id: u64,
    },
    RecoverEditSave {
        save_id: u64,
    },
    ReleaseEditSave {
        save_id: u64,
    },
    RetryInsert {
        save_id: u64,
    },
    CancelInsertRetry {
        operation_id: u64,
    },
    DiscardRetainedInsert {
        save_id: u64,
    },
    AcknowledgeEditSave {
        save_id: u64,
    },
    DescribeQueryDesign {
        request: QueryDesignRequest,
        table: String,
    },
    BuildQueryDesign {
        request: QueryDesignRequest,
        draft: tablecraft_builders::query::QueryDesignDraft,
        completion: QueryDesignCompletion,
    },
    ImportQueryDesign {
        request: QueryDesignRequest,
        sql: String,
    },
    CancelQueryDesign {
        request_id: u64,
    },
    Query,
    Cancel,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum DocumentView {
    #[default]
    Table,
    Query,
    Design,
}

/// Presentation state shared by potential native and browser hosts.
#[derive(Default)]
pub struct State {
    pub localizer: crate::Localizer,
    pub theme: crate::Theme,
    pub font_families: crate::FontFamilies,
    pub formatting: Options,
    format_cache: crate::formatting::Cache,
    /// Synthetic presentation only; never grants database capabilities.
    pub mock_preview: bool,
    pub path: String,
    pub password: String,
    /// The host has an actual file-selection implementation.
    pub file_picker_available: bool,
    /// Host supports fresh native publication; synthetic/default hosts keep false.
    pub creation_available: bool,
    creation: creation::Creation,
    pub database: Option<Database>,
    /// Last table page; query results have a separate slot.
    pub data: Option<Data>,
    pub query_data: Option<Data>,
    query_result: ResultOrigin,
    query_pending: Option<PendingSql>,
    query_sql_revision: u64,
    query_parameters: Option<query::parameters::scratch::Session>,
    query_action: Option<query::actions::review::Session>,
    table_rename: Option<navigation::rename::Rename>,
    /// Host opt-in remains false until the checked schema-reopen controller is connected.
    pub field_addition_available: bool,
    field_addition: field_addition::Model,
    /// Host opt-in stays false until the guarded rename controller is connected.
    pub field_rename_available: bool,
    field_rename: field_rename::model::Model,
    database_options: database_options::Model,
    pub database_options_available: bool,
    database_options_confirmation_sequence: u64,
    options_open: bool,
    options_current_database: bool,
    field_addition_status: Option<schema_save::Status<FieldAdditionSaveIdentity>>,
    /// Host can review and persist native table renames; false for synthetic hosts.
    pub table_rename_available: bool,
    table_selection: Selection,
    query_selection: Selection,
    pub schema: Option<Schema>,
    contents: Option<contents::ContentsDialog>,
    recovery: Option<recovery::Recovery>,
    edit: Option<editing::EditDialog>,
    paste_request: Option<editing::paste::ClipboardRequest>,
    ribbon_paste: Option<home::menus::paste::PendingPaste>,
    insertion: Option<insertion::Dialog>,
    insert_continuation: Option<insertion::continuation::Continuation>,
    initial_record_notice: Option<InsertTarget>,
    deletion: Option<deletion::Dialog>,
    /// Host capability; false until the complete native Find service is ready.
    pub find_available: bool,
    pub replace_available: bool,
    /// Host-qualified formatted-search capability, separate from raw Find.
    pub find_formatted_available: bool,
    find: Option<find::Session>,
    table_filter: filtering::TableFilter,
    totals: totals::Totals,
    zoom: zoom::Zoom,
    records: records::RecordNavigation,
    /// Host lifecycle readiness; native eligibility alone never enables editing.
    pub saved_query_sql_edit_available: bool,
    saved_query: saved_query::SavedQueryState,
    active_save: active_save::PendingSave,
    saved_query_sql_close: Option<saved_query::SqlCloseConfirmation>,
    saved_query_sql_save: Option<saved_query_save::SaveStatus>,
    selected_field: usize,
    design_focus_pending: Option<schema::PaneEntry>,
    pane_focus_entry: Option<focus::Entry>,
    focus_publication: u64,
    schema_activation_pending: bool,
    pub busy: bool,
    pub error: Option<String>,
    save_cleanup_notice: Option<(u64, SaveCleanupNotice)>,
    pub sql: String,
    query_design: query_design::QueryDesign,
    filter: String,
    pub table_save_available: bool,
    table_creation: Option<u64>,
    table_design: Option<u64>,
    table_draft_save: Option<table_draft::save::SaveDialog>,
    table_draft_field_delete: Option<table_draft::field_delete::Confirmation>,
    table_draft: Option<table_draft::Editor>,
    table_draft_close: Option<table_draft::CloseIntent>,
    table_draft_discard_approved: Option<(ObjectTab, u64, u64, u64)>,
    view: DocumentView,
    ribbon_tab: ribbon::Tab,
    ribbon_display: chrome::ribbon_display::RibbonDisplay,
    ribbon_keyboard: ribbon::keyboard::Keyboard,
    show_system: bool,
    objects: objects::Objects,
    navigation_hidden: bool,
    navigation_clicks: navigation::activation::Clicks,
    backstage: backstage::Backstage,
}

impl State {
    fn prepare_display(&mut self, data: &mut Data) {
        self.refresh_display();
        self.format_cache.apply_data(data);
    }

    fn refresh_display(&mut self) {
        if !self.format_cache.update(&self.formatting) {
            return;
        }

        if let Some(data) = &mut self.data {
            self.format_cache.apply_data(data);
        }
        if let Some(data) = &mut self.query_data {
            self.format_cache.apply_data(data);
        }
        if let Some(data) = &mut self.saved_query.data {
            self.format_cache.apply_data(data);
        }
        self.refresh_contents_display();
        self.refresh_edit_display();
        self.refresh_delete_display();
        self.refresh_totals_display();
        self.refresh_parked_totals_display();
    }

    /// Publish a checked table page into its admitted object session.
    pub fn set_table_data(&mut self, mut data: Data) -> bool {
        if data.validate_row_ordinals().is_err() {
            self.error = Some(self.localizer.text("datasheet-row-identity-invalid"));
            return false;
        }
        if !self.admit_object_data(&data) {
            return false;
        }
        self.prepare_display(&mut data);
        if !self.admit_object_data(&data) || !self.adopt_table_publication(&data) {
            return false;
        }

        self.reset_table_filter();
        self.bump_focus_publication();
        self.reset_record_navigation();
        self.invalidate_edit();
        self.close_contents();
        self.table_selection.reset(&data);
        self.data = Some(data);
        true
    }

    /// Publish a result and its captured submitted SQL into the scratch object.
    /// `None` retains unknown-origin data without claiming draft equivalence.
    pub fn set_query_data(&mut self, submitted_sql: Option<String>, mut data: Data) -> bool {
        if data.source_revision.is_some() || data.row_ordinals.is_some() {
            self.error = Some(self.localizer.text("datasheet-row-identity-invalid"));
            return false;
        }
        if !self.admit_query_data(&submitted_sql, &data) {
            return false;
        }
        self.prepare_display(&mut data);
        if !self.admit_query_data(&submitted_sql, &data) || !self.adopt_sql_publication() {
            return false;
        }

        self.bump_focus_publication();
        self.reset_record_navigation();
        self.invalidate_edit();
        self.query_selection.reset(&data);
        self.query_data = Some(data);
        self.query_result = submitted_sql.map_or(ResultOrigin::Unknown, ResultOrigin::Sql);
        true
    }

    /// Full text of the visible single-cell selection; record selection is separate.
    pub fn selected_cell_text(&self) -> Option<&str> {
        if self.backstage_visible() || self.edit_dialog_open() {
            return None;
        }
        if self.contents_request_id().is_some() {
            return self.selected_contents_text();
        }
        if self.total_selection_active() {
            return self.selected_total_cell().map(|cell| cell.text.as_str());
        }
        let data = self.visible_data()?;
        match self.view {
            DocumentView::Table => self.table_selection.text(data),
            DocumentView::Query if self.saved_query.active => self.saved_query.selection.text(data),
            DocumentView::Query => self.query_selection.text(data),
            DocumentView::Design => None,
        }
    }

    pub fn can_copy_selection(&self) -> bool {
        if self.backstage_visible() || self.edit_dialog_open() {
            return false;
        }
        if self.contents_request_id().is_some() {
            return self.can_copy_contents_selection();
        }
        if self.total_selection_active() {
            return self.selected_total_cell().is_some();
        }
        let Some(data) = self.visible_data() else {
            return false;
        };
        match self.view {
            DocumentView::Table => self.table_selection.can_copy(data),
            DocumentView::Query if self.saved_query.active => {
                self.saved_query.selection.can_copy(data)
            }
            DocumentView::Query => self.query_selection.can_copy(data),
            DocumentView::Design => false,
        }
    }

    fn record_selected(&self) -> bool {
        if self.query_design_visible()
            || self.total_selection_active()
            || self.backstage_visible()
            || self.edit_dialog_open()
            || self.contents_request_id().is_some()
        {
            return false;
        }
        match self.view {
            DocumentView::Table => self.table_selection.row_selected(),
            DocumentView::Query if self.saved_query.active => {
                self.saved_query.selection.row_selected()
            }
            DocumentView::Query => self.query_selection.row_selected(),
            DocumentView::Design => false,
        }
    }

    pub fn copy_selected(&mut self, ctx: &egui::Context) -> bool {
        if self.backstage_visible() || self.edit_dialog_open() || self.query_design_visible() {
            return false;
        }
        if self.contents_request_id().is_some() {
            return self.copy_contents_selection(ctx);
        }
        if self.total_selection_active() {
            let Some(cell) = self.selected_total_cell() else {
                return false;
            };
            ctx.copy_text(cell.text.clone());
            return true;
        }
        match self.view {
            DocumentView::Table => self
                .data
                .as_ref()
                .is_some_and(|data| self.table_selection.copy(data, ctx)),
            DocumentView::Query if self.saved_query.active => self
                .saved_query
                .data
                .as_ref()
                .is_some_and(|data| self.saved_query.selection.copy(data, ctx)),
            DocumentView::Query => self
                .query_data
                .as_ref()
                .is_some_and(|data| self.query_selection.copy(data, ctx)),
            DocumentView::Design => false,
        }
    }

    /// Record explicit navigation to Design before the host submits Describe.
    pub fn prepare_schema_view(&mut self) {
        if self.table_draft_field_delete_open() {
            return;
        }
        self.invalidate_edit();
        self.reset_record_navigation();
        self.schema_activation_pending = true;
    }

    /// Publish metadata without overriding a view chosen after its request.
    pub fn set_schema(&mut self, schema: Schema) {
        let activate = self.schema_activation_pending;
        if !self.admit_object_schema(&schema) || !self.adopt_schema_publication(&schema) {
            return;
        }
        self.schema_activation_pending = activate;
        self.bump_focus_publication();
        self.close_contents();
        self.schema = Some(schema);
        self.selected_field = 0;
        if std::mem::take(&mut self.schema_activation_pending) {
            self.interrupt_saved_query();
            self.view = DocumentView::Design;
            self.ribbon_tab = ribbon::Tab::TableDesign;
        }
    }

    fn select_view(&mut self, view: DocumentView) {
        if self.edit_save_pending()
            || self.insertion_dialog_open()
            || self.recovery_prompt_open()
            || self.table_draft_field_delete_open()
        {
            return;
        }
        self.invalidate_edit();
        self.reset_record_navigation();
        self.interrupt_saved_query();
        self.close_contents();
        self.schema_activation_pending = false;
        self.view = view;
        self.ribbon_tab = match view {
            DocumentView::Design => ribbon::Tab::TableDesign,
            DocumentView::Table | DocumentView::Query => ribbon::Tab::Home,
        };
    }

    /// Remove metadata when the host replaces or closes its source.
    pub fn clear_schema(&mut self) {
        if self.has_modified_saved_query_sql()
            || self.insertion_dialog_open()
            || self.recovery_prompt_open()
            || self.table_draft_field_delete_open()
        {
            return;
        }
        self.clear_object_sessions();
        self.bump_focus_publication();
        self.reset_saved_query();
        self.reset_record_navigation();
        self.invalidate_edit();
        self.close_contents();
        self.table_selection.clear();
        self.query_selection.clear();
        self.schema = None;
        self.selected_field = 0;
        self.schema_activation_pending = false;
        if self.view == DocumentView::Design {
            self.view = DocumentView::Table;
            self.ribbon_tab = ribbon::Tab::Home;
        }
    }

    /// Enter File/Open without changing the current session or running task.
    pub fn show_open(&mut self) {
        if self.edit_save_pending()
            || self.insertion_dialog_open()
            || self.recovery_prompt_open()
            || self.table_draft_field_delete_open()
        {
            return;
        }
        self.invalidate_edit();
        self.reset_record_navigation();
        self.interrupt_saved_query();
        self.backstage = backstage::Backstage::Open;
    }

    /// The host dismisses File/Open only after a successful Opened response.
    pub fn dismiss_backstage(&mut self) {
        self.backstage = backstage::Backstage::Closed;
    }

    pub fn backstage_visible(&self) -> bool {
        self.backstage != backstage::Backstage::Closed
    }

    fn file_picker_ready(&self) -> bool {
        self.file_picker_available
            && !self.busy
            && !self.table_draft_field_delete_open()
            && !self.query_action_dialog_open()
            && !self.table_rename_open()
    }

    fn query_ready(&self) -> bool {
        self.view == DocumentView::Query
            && !self.saved_query.active
            && !self.query_design_visible()
            && !self.query_design_results_visible()
            && !self.backstage_visible()
            && self.database.is_some()
            && !self.busy
            && self.query_parameters.is_none()
            && !self.query_action_dialog_open()
            && !self.table_rename_open()
            && !self.sql.trim().is_empty()
    }

    fn visible_data(&self) -> Option<&Data> {
        if self.query_design_visible() {
            return None;
        }
        match self.view {
            DocumentView::Table => self.data.as_ref(),
            DocumentView::Query if self.saved_query.active => self.saved_query.data.as_ref(),
            DocumentView::Query => self.query_data.as_ref(),
            DocumentView::Design => None,
        }
    }

    /// Draw the workspace and return explicit user actions; performs no I/O.
    pub fn show(&mut self, ui: &mut Ui) -> Vec<Action> {
        self.synchronize_objects();
        self.validate_query_parameter_acquisition();
        self.validate_query_action();
        self.refresh_display();
        self.refresh_edit_display();
        self.refresh_delete_display();
        self.validate_zoom();
        self.validate_table_filter();
        self.validate_field_filter();
        self.validate_totals();
        crate::localization::install(ui.ctx(), &self.localizer);
        crate::theme::set_theme(ui.ctx(), self.theme);
        crate::fonts::configure(ui.ctx(), self.localizer.locale());
        let mut actions = Vec::new();
        let object_shortcut = self.begin_object_shortcut(ui);
        let paste_receipt = self.receive_requested_paste(ui.ctx(), &mut actions);
        self.consume_active_save_shortcut(ui.ctx());
        let modal_open = self.options_dialog_open()
            || self.database_options_modal_open()
            || self.field_addition_close_open()
            || self.field_addition_modal_open()
            || self.field_rename_modal_open()
            || self.field_rename_close_open()
            || self.saved_query_sql_save_open()
            || self.table_draft_save.is_some()
            || self.table_draft_field_delete_open()
            || self.insert_continuation_pending()
            || self.table_draft_close.is_some()
            || self.contents_request_id().is_some()
            || self.edit_dialog_open()
            || self.parameter_prompt_open()
            || self.query_action_dialog_open()
            || self.table_rename_open()
            || self.zoom_dialog_open();
        self.prepare_ribbon_display(ui, modal_open);
        self.begin_ribbon_keyboard(ui, modal_open);
        if !modal_open
            && self.backstage_visible()
            && self.backstage_return_available()
            && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.return_from_backstage();
        }
        let shortcuts = if modal_open || self.inline_insert_active() {
            keyboard::Shortcuts::default()
        } else {
            keyboard::consume(
                ui.ctx(),
                !self.find_window_open()
                    && (self.query_ready()
                        || self.query_design_run_action().is_some()
                        || self.saved_query_command_ready()),
                self.busy && !self.find_window_open() && !self.ribbon_temporarily_open(),
                self.can_open_find(),
            )
        };
        if shortcuts.find {
            self.open_find();
        }
        if shortcuts.query {
            if self.saved_query_visible() {
                if let Some(action) = self.request_saved_query_run() {
                    actions.push(action);
                }
            } else if let Some(action) = self.query_design_run_action() {
                actions.push(action);
            } else {
                actions.push(Action::Query);
            }
        }
        if shortcuts.cancel {
            actions.push(Action::Cancel);
        }
        if !modal_open
            && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::F11))
        {
            self.toggle_navigation();
        }
        let path_id = egui::Id::new("database_path");
        if shortcuts.focus_path {
            if self.file_picker_ready() {
                actions.push(Action::ChooseFile);
            } else if !self.file_picker_available {
                self.show_open();
                ui.memory_mut(|memory| memory.request_focus(path_id));
            }
        }
        let modal_open = modal_open
            || self.field_addition_modal_open()
            || self.field_rename_modal_open()
            || self.field_rename_close_open()
            || self.parameter_prompt_open()
            || self.query_action_dialog_open()
            || self.table_rename_open()
            || self.zoom_dialog_open();
        self.begin_pane_focus(ui, modal_open || self.find_window_open());
        ui.add_enabled_ui(!modal_open, |ui| {
            self.chrome(ui, &mut actions);
            ui.add_enabled_ui(!self.zoom_dialog_open(), |ui| {
                if self.backstage_visible() {
                    self.backstage(ui, &mut actions, path_id);
                } else {
                    self.navigation(ui, &mut actions);
                    self.content(ui, &mut actions);
                }
            });
        });
        self.finish_active_save(&mut actions);
        self.finish_field_rename_inspection(&mut actions);
        self.new_record_shortcut(ui, &mut actions);
        self.finish_ribbon_paste(ui, &actions);
        self.finish_paste(ui.ctx(), &mut actions, paste_receipt);
        self.accept_zoom_request(ui.ctx());
        self.find_window(ui.ctx(), &mut actions);
        self.contents_dialog(ui.ctx(), &mut actions);
        self.edit_dialog(ui.ctx(), &mut actions);
        self.insertion_dialog(ui.ctx(), &mut actions);
        self.insert_continuation_dialog(ui.ctx(), &mut actions);
        self.deletion_dialog(ui.ctx(), &mut actions);
        self.saved_query_prompt_dialog(ui.ctx(), &mut actions);
        self.scratch_parameter_dialog(ui.ctx(), &mut actions);
        self.query_action_dialog(ui.ctx(), &mut actions);
        self.table_rename_dialog(ui.ctx(), &mut actions);
        self.zoom_dialog(ui.ctx(), &mut actions);
        self.field_filter_dialog(ui.ctx(), &mut actions);
        self.query_design_dialog(ui.ctx(), &mut actions);
        self.table_draft_close_dialog(ui.ctx(), &mut actions);
        self.saved_query_sql_close_dialog(ui.ctx(), &mut actions);
        self.saved_query_sql_save_dialog(ui.ctx(), &mut actions);
        self.options_dialog(ui.ctx(), &mut actions);
        self.database_options_dialog(ui.ctx(), &mut actions);
        self.field_addition_close_dialog(ui.ctx(), &mut actions);
        self.field_addition_save_dialog(ui.ctx(), &mut actions);
        self.field_rename_close_dialog(ui.ctx(), &mut actions);
        self.field_rename_save_dialog(ui.ctx(), &mut actions);
        self.table_draft_save_dialog(ui.ctx(), &mut actions);
        self.table_draft_field_delete_dialog(ui.ctx());
        self.recovery_dialog(ui.ctx(), &mut actions);
        if !self.parameter_prompt_open()
            && !self.query_action_dialog_open()
            && !self.table_rename_open()
            && !self.zoom_dialog_open()
            && !self.find_window_open()
            && self.table_draft_save.is_none()
            && self.table_draft_close.is_none()
            && !self.table_draft_field_delete_open()
            && !self.recovery_prompt_open()
        {
            self.finish_ribbon_keyboard(ui, !actions.is_empty());
            self.finish_pane_focus(ui);
            self.finish_ribbon_display(ui);
        }
        self.capture_ribbon_focus(ui);
        self.finish_object_shortcut(ui, object_shortcut, &mut actions);
        if !self.recovery_prompt_open()
            && actions.is_empty()
            && let Some(action) = self.take_totals_request()
        {
            actions.push(action);
        }
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(table: &str, offset: u32) -> Data {
        Data {
            row_ordinals: None,
            source_revision: None,
            table: table.into(),
            offset,
            columns: vec!["Id".into()],
            rows: vec![vec!["1".into()]],
            has_more: true,
        }
    }

    #[test]
    fn tab_results_retain_separate_data_and_original_table_page_offset() {
        let mut state = State::default();
        state.set_table_data(data("Customers", 200));
        assert_eq!(state.visible_data().unwrap().table, "Customers");
        state.view = DocumentView::Query;
        assert!(state.visible_data().is_none());
        state.set_query_data(None, data("Query result", 0));
        assert_eq!(state.visible_data().unwrap().table, "Query result");
        state.view = DocumentView::Table;
        let page = state.visible_data().unwrap();
        assert_eq!(page.table, "Customers");
        assert_eq!(page.offset, 200);
        state.set_table_data(data("Customers", 300));
        state.view = DocumentView::Query;
        assert_eq!(state.visible_data().unwrap().table, "Query result");
        assert_eq!(state.visible_data().unwrap().offset, 0);
    }

    #[test]
    fn running_query_requires_query_tab_document_text_and_idle_state() {
        let mut state = State {
            sql: "SELECT 1".into(),
            ..Default::default()
        };
        assert!(!state.query_ready());
        state.database = Some(Database {
            title_path: None,
            document_revision: 1,
            title: "Sample".into(),
            format: "ACE16".into(),
            queries: vec![],
            tables: vec![],
        });
        assert!(!state.query_ready());
        state.view = DocumentView::Query;
        state.dismiss_backstage();
        assert!(state.query_ready());
        state.busy = true;
        assert!(!state.query_ready());
        state.busy = false;
        state.sql.clear();
        assert!(!state.query_ready());
    }
}

#[cfg(test)]
mod picker_tests {
    use super::*;

    fn command_open(state: &mut State) -> (Vec<Action>, Option<egui::Id>) {
        let ctx = egui::Context::default();
        let mut observed = None;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1280.0, 820.0),
                )),
                events: vec![egui::Event::Key {
                    key: egui::Key::O,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers {
                        ctrl: true,
                        command: true,
                        ..Default::default()
                    },
                }],
                ..Default::default()
            },
            |ui| {
                let actions = state.show(ui);
                let focused = ui.memory(|memory| memory.focused());
                observed = Some((actions, focused));
            },
        );
        output.textures_delta.clear();
        observed.unwrap()
    }

    #[test]
    fn command_open_uses_host_picker_or_focuses_the_existing_path_field() {
        let mut state = State::default();
        let (actions, focused) = command_open(&mut state);
        assert!(actions.is_empty());
        assert_eq!(focused, Some(egui::Id::new("database_path")));
        state.file_picker_available = true;
        let (actions, _) = command_open(&mut state);
        assert!(matches!(actions.as_slice(), [Action::ChooseFile]));
    }

    #[test]
    fn busy_picker_capability_disables_browse_and_command_open() {
        let mut state = State {
            file_picker_available: true,
            busy: true,
            ..Default::default()
        };
        assert!(!state.file_picker_ready());
        let (actions, _) = command_open(&mut state);
        assert!(actions.is_empty());
        state.busy = false;
        assert!(state.file_picker_ready());
        let (actions, _) = command_open(&mut state);
        assert!(matches!(actions.as_slice(), [Action::ChooseFile]));
    }
}

#[cfg(test)]
mod selection_tests {
    use super::*;

    fn data(text: &str) -> Data {
        Data {
            row_ordinals: None,
            source_revision: None,
            table: "Same result name".into(),
            offset: 0,
            columns: vec!["Value".into()],
            rows: vec![vec![text.into()]],
            has_more: false,
        }
    }

    #[test]
    fn publication_resets_only_its_slot_and_hidden_views_do_not_publish_copy_text() {
        let mut state = State::default();
        state.dismiss_backstage();
        state.set_table_data(data("old table"));
        state.set_query_data(None, data("old query"));
        assert_eq!(state.selected_cell_text(), Some("old table"));
        state.select_view(DocumentView::Query);
        assert_eq!(state.selected_cell_text(), Some("old query"));
        state.set_table_data(data("new table"));
        assert_eq!(state.selected_cell_text(), Some("old query"));
        state.set_query_data(None, data("new query"));
        assert_eq!(state.selected_cell_text(), Some("new query"));
        state.select_view(DocumentView::Design);
        assert!(state.selected_cell_text().is_none());
        state.select_view(DocumentView::Table);
        assert_eq!(state.selected_cell_text(), Some("new table"));
        state.show_open();
        assert!(state.selected_cell_text().is_none());
        state.dismiss_backstage();
        assert_eq!(state.selected_cell_text(), Some("new table"));
        state.clear_schema();
        assert!(state.selected_cell_text().is_none());
        state.data = None;
        assert!(state.selected_cell_text().is_none());
    }
}
