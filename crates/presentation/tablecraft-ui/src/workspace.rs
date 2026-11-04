mod backstage;
mod chrome;
mod content;
mod contents;
mod editing;
mod home;
mod navigation;
mod query;
mod records;
mod schema;

pub use contents::{CollectionKind, ContentsTarget};
pub use editing::{EditDraft, EditInput, EditReview, EditSaveOutcome, EditTarget};
pub use records::RecordTarget;
pub use schema::{Field, Schema};

use super::{Cell, keyboard};
use crate::datasheet::{self, Selection};
use egui::Ui;

/// One inert catalog entry. A visible declaration does not imply readable rows.
pub struct Table {
    pub name: String,
    pub system: bool,
    pub storage: String,
}

/// Opened database metadata supplied by the application host.
pub struct Database {
    pub document_revision: u64,
    pub title: String,
    pub format: String,
    pub tables: Vec<Table>,
}

/// One bounded buffered page or query result supplied by the host.
pub struct Data {
    pub source_revision: Option<u64>,
    pub table: String,
    pub offset: u32,
    pub columns: Vec<String>,
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
    BeginEdit {
        target: EditTarget,
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
    pub path: String,
    pub password: String,
    /// The host has an actual file-selection implementation.
    pub file_picker_available: bool,
    pub database: Option<Database>,
    /// Last table page; query results have a separate slot.
    pub data: Option<Data>,
    pub query_data: Option<Data>,
    table_selection: Selection,
    query_selection: Selection,
    pub schema: Option<Schema>,
    contents: Option<contents::ContentsDialog>,
    edit: Option<editing::EditDialog>,
    records: records::RecordNavigation,
    selected_field: usize,
    schema_activation_pending: bool,
    pub busy: bool,
    pub error: Option<String>,
    pub sql: String,
    filter: String,
    view: DocumentView,
    show_system: bool,
    backstage: backstage::Backstage,
}

impl State {
    /// Publish a table response without replacing the query result.
    pub fn set_table_data(&mut self, data: Data) {
        self.reset_record_navigation();
        self.invalidate_edit();
        self.close_contents();
        self.table_selection.reset(&data);
        self.data = Some(data);
    }

    /// Publish a query response without changing the table page or its offset.
    pub fn set_query_data(&mut self, data: Data) {
        self.reset_record_navigation();
        self.invalidate_edit();
        self.query_selection.reset(&data);
        self.query_data = Some(data);
    }

    /// Full text of the retained selection in the visible loaded data only.
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
            DocumentView::Query => self.query_selection.text(data),
            DocumentView::Design => None,
        }
    }

    pub fn copy_selected(&self, ctx: &egui::Context) -> bool {
        let Some(text) = self.selected_cell_text() else {
            return false;
        };
        datasheet::copy_text(ctx, text);
        true
    }

    /// Record explicit navigation to Design before the host submits Describe.
    pub fn prepare_schema_view(&mut self) {
        self.invalidate_edit();
        self.reset_record_navigation();
        self.schema_activation_pending = true;
    }

    /// Publish metadata without overriding a view chosen after its request.
    pub fn set_schema(&mut self, schema: Schema) {
        self.close_contents();
        self.schema = Some(schema);
        self.selected_field = 0;
        if std::mem::take(&mut self.schema_activation_pending) {
            self.view = DocumentView::Design;
        }
    }

    fn select_view(&mut self, view: DocumentView) {
        if self.edit_save_pending() {
            return;
        }
        self.invalidate_edit();
        self.reset_record_navigation();
        self.close_contents();
        self.schema_activation_pending = false;
        self.view = view;
    }

    /// Remove metadata when the host replaces or closes its source.
    pub fn clear_schema(&mut self) {
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
        }
    }

    /// Enter File/Open without changing the current session or running task.
    pub fn show_open(&mut self) {
        if self.edit_save_pending() {
            return;
        }
        self.invalidate_edit();
        self.reset_record_navigation();
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
            && !self.backstage_visible()
            && self.database.is_some()
            && !self.busy
            && !self.sql.trim().is_empty()
    }

    fn visible_data(&self) -> Option<&Data> {
        match self.view {
            DocumentView::Table => self.data.as_ref(),
            DocumentView::Query => self.query_data.as_ref(),
            DocumentView::Design => None,
        }
    }

    /// Draw the workspace and return explicit user actions; performs no I/O.
    pub fn show(&mut self, ui: &mut Ui) -> Vec<Action> {
        let mut actions = Vec::new();
        let modal_open = self.contents_request_id().is_some() || self.edit_dialog_open();
        if !modal_open
            && self.backstage_visible()
            && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.dismiss_backstage();
        }
        let shortcuts = if modal_open {
            keyboard::Shortcuts::default()
        } else {
            keyboard::consume(ui.ctx(), self.query_ready(), self.busy)
        };
        if shortcuts.query {
            actions.push(Action::Query);
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
            document_revision: 1,
            title: "Sample".into(),
            format: "ACE16".into(),
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
