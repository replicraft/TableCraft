mod backstage;
mod chrome;
mod content;
mod contents;
mod deletion;
mod diagnostics;
mod editing;
pub(crate) mod focus;
mod home;
mod insertion;
mod navigation;
pub mod preview;
mod query;
mod records;
mod ribbon;
mod save_display;
mod saved_query;
mod schema;

pub use contents::{CollectionKind, ContentsTarget};
pub use deletion::{DeleteReview, DeleteTarget};
pub use editing::{EditDraft, EditInput, EditReview, EditSaveOutcome, EditTarget, InputHint};
pub use insertion::{InsertDraft, InsertField, InsertInput, InsertReview, InsertTarget};
pub use records::RecordTarget;
pub use saved_query::{ParameterInput, ParameterSpec, SavedQuery, SavedQueryTarget};
pub use schema::{Field, FieldProperty, FieldPropertyValue, Schema};

use super::{Cell, Column, keyboard};
use crate::datasheet::Selection;
use egui::Ui;

/// One inert catalog entry. A visible declaration does not imply readable rows.
pub struct Table {
    pub name: String,
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
    pub table: String,
    pub offset: u32,
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Cell>>,
    pub has_more: bool,
}

/// User intents dispatched to the host's shared backend services.
pub enum Action {
    ChooseFile,
    Open,
    Page {
        table: String,
        offset: u32,
    },
    InspectSavedQuery {
        target: SavedQueryTarget,
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
    CloseTable {
        document_revision: u64,
        table: String,
    },
    SortTable {
        document_revision: u64,
        table: String,
        column: String,
        descending: bool,
    },
    BeginEdit {
        target: EditTarget,
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
    AcknowledgeEditSave {
        save_id: u64,
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
    pub formatting: crate::formatting::Options,
    format_cache: crate::formatting::Cache,
    /// Synthetic presentation only; never grants database capabilities.
    pub mock_preview: bool,
    pub path: String,
    pub password: String,
    /// The host has an actual file-selection implementation.
    pub file_picker_available: bool,
    pub database: Option<Database>,
    /// Last table page; query results have a separate slot.
    pub data: Option<Data>,
    pub query_data: Option<Data>,
    sorted_query: bool,
    table_selection: Selection,
    query_selection: Selection,
    pub schema: Option<Schema>,
    contents: Option<contents::ContentsDialog>,
    edit: Option<editing::EditDialog>,
    insertion: Option<insertion::Dialog>,
    deletion: Option<deletion::Dialog>,
    records: records::RecordNavigation,
    saved_query: saved_query::SavedQueryState,
    selected_field: usize,
    design_focus_pending: Option<schema::PaneEntry>,
    pane_focus_entry: Option<focus::Entry>,
    focus_publication: u64,
    schema_activation_pending: bool,
    pub busy: bool,
    pub error: Option<String>,
    pub sql: String,
    filter: String,
    view: DocumentView,
    ribbon_tab: ribbon::Tab,
    show_system: bool,
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
    }

    /// Publish a table response without replacing the query result.
    pub fn set_table_data(&mut self, mut data: Data) {
        self.bump_focus_publication();
        self.reset_record_navigation();
        self.invalidate_edit();
        self.close_contents();
        self.prepare_display(&mut data);
        self.table_selection.reset(&data);
        self.data = Some(data);
    }

    /// Publish a query response without changing the table page or its offset.
    pub fn set_query_data(&mut self, mut data: Data) {
        self.sorted_query = false;
        self.bump_focus_publication();
        self.reset_record_navigation();
        self.invalidate_edit();
        self.prepare_display(&mut data);
        self.query_selection.reset(&data);
        self.query_data = Some(data);
    }

    /// Full text of the visible single-cell selection; record selection is separate.
    pub fn selected_cell_text(&self) -> Option<&str> {
        if self.backstage_visible() || self.edit_dialog_open() {
            return None;
        }
        if self.contents_request_id().is_some() {
            return self.selected_contents_text();
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
        if self.backstage_visible()
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
        if self.backstage_visible() || self.edit_dialog_open() {
            return false;
        }
        if self.contents_request_id().is_some() {
            return self.copy_contents_selection(ctx);
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
        self.invalidate_edit();
        self.reset_record_navigation();
        self.schema_activation_pending = true;
    }

    /// Publish metadata without overriding a view chosen after its request.
    pub fn set_schema(&mut self, schema: Schema) {
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
        if self.edit_save_pending() {
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
        if self.edit_save_pending() {
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
        self.file_picker_available && !self.busy
    }

    fn query_ready(&self) -> bool {
        self.view == DocumentView::Query
            && !(self.sorted_query && self.query_data.is_some())
            && !self.saved_query.active
            && !self.backstage_visible()
            && self.database.is_some()
            && !self.busy
            && !self.sql.trim().is_empty()
    }

    fn visible_data(&self) -> Option<&Data> {
        match self.view {
            DocumentView::Table => self.data.as_ref(),
            DocumentView::Query if self.saved_query.active => self.saved_query.data.as_ref(),
            DocumentView::Query => self.query_data.as_ref(),
            DocumentView::Design => None,
        }
    }

    /// Draw the workspace and return explicit user actions; performs no I/O.
    pub fn show(&mut self, ui: &mut Ui) -> Vec<Action> {
        self.refresh_display();
        self.refresh_edit_display();
        self.refresh_delete_display();
        crate::localization::install(ui.ctx(), &self.localizer);
        crate::theme::set_theme(ui.ctx(), self.theme);
        crate::fonts::configure(ui.ctx(), self.localizer.locale());
        let mut actions = Vec::new();
        let modal_open = self.contents_request_id().is_some()
            || self.edit_dialog_open()
            || self.saved_query_prompt_open();
        if !modal_open
            && self.backstage_visible()
            && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.dismiss_backstage();
        }
        let shortcuts = if modal_open {
            keyboard::Shortcuts::default()
        } else {
            keyboard::consume(
                ui.ctx(),
                self.query_ready() || self.saved_query_command_ready(),
                self.busy,
            )
        };
        if shortcuts.query {
            if self.saved_query_visible() {
                if let Some(action) = self.request_saved_query_run() {
                    actions.push(action);
                }
            } else {
                actions.push(Action::Query);
            }
        }
        if shortcuts.cancel {
            actions.push(Action::Cancel);
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
        let modal_open = modal_open || self.saved_query_prompt_open();
        self.begin_pane_focus(ui, modal_open);
        ui.add_enabled_ui(!modal_open, |ui| {
            self.chrome(ui, &mut actions);
            if self.backstage_visible() {
                self.backstage(ui, &mut actions, path_id);
            } else {
                self.navigation(ui, &mut actions);
                self.content(ui, &mut actions);
            }
        });
        self.contents_dialog(ui.ctx(), &mut actions);
        self.edit_dialog(ui.ctx(), &mut actions);
        self.insertion_dialog(ui.ctx(), &mut actions);
        self.deletion_dialog(ui.ctx(), &mut actions);
        self.saved_query_prompt_dialog(ui.ctx(), &mut actions);
        if !self.saved_query_prompt_open() {
            self.finish_pane_focus(ui);
        }
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(table: &str, offset: u32) -> Data {
        Data {
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
        state.set_query_data(data("Query result", 0));
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
        state.set_query_data(data("old query"));
        assert_eq!(state.selected_cell_text(), Some("old table"));
        state.select_view(DocumentView::Query);
        assert_eq!(state.selected_cell_text(), Some("old query"));
        state.set_table_data(data("new table"));
        assert_eq!(state.selected_cell_text(), Some("old query"));
        state.set_query_data(data("new query"));
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
