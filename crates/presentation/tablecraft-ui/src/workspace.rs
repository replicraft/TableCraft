mod chrome;
mod content;
mod navigation;
mod query;

use super::keyboard;
use egui::Ui;

/// One inert catalog entry. A visible declaration does not imply readable rows.
pub struct Table {
    pub name: String,
    pub system: bool,
    pub storage: String,
}

/// Opened database metadata supplied by the application host.
pub struct Database {
    pub title: String,
    pub format: String,
    pub tables: Vec<Table>,
}

/// One bounded buffered page or query result supplied by the host.
pub struct Data {
    pub table: String,
    pub offset: u32,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub has_more: bool,
}

/// User intents dispatched to the host's shared backend services.
pub enum Action {
    ChooseFile,
    Open,
    Page { table: String, offset: u32 },
    Query,
    Cancel,
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
    pub busy: bool,
    pub error: Option<String>,
    pub sql: String,
    filter: String,
    query_view: bool,
    show_system: bool,
    about: bool,
}

impl State {
    /// Publish a table response without replacing the query result.
    pub fn set_table_data(&mut self, data: Data) {
        self.data = Some(data);
    }

    /// Publish a query response without changing the table page or its offset.
    pub fn set_query_data(&mut self, data: Data) {
        self.query_data = Some(data);
    }

    fn file_picker_ready(&self) -> bool {
        self.file_picker_available && !self.busy
    }

    fn query_ready(&self) -> bool {
        self.query_view && self.database.is_some() && !self.busy && !self.sql.trim().is_empty()
    }

    fn visible_data(&self) -> Option<&Data> {
        if self.query_view {
            self.query_data.as_ref()
        } else {
            self.data.as_ref()
        }
    }

    /// Draw the workspace and return explicit user actions; performs no I/O.
    pub fn show(&mut self, ui: &mut Ui) -> Vec<Action> {
        let mut actions = Vec::new();
        let shortcuts = keyboard::consume(ui.ctx(), self.query_ready(), self.busy);
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
                ui.memory_mut(|memory| memory.request_focus(path_id));
            }
        }
        self.chrome(ui, &mut actions, path_id);
        self.navigation(ui, &mut actions);
        self.content(ui, &mut actions);
        self.about_window(ui);
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(table: &str, offset: u32) -> Data {
        Data {
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
        state.query_view = true;
        assert!(state.visible_data().is_none());
        state.set_query_data(data("Query result", 0));
        assert_eq!(state.visible_data().unwrap().table, "Query result");
        state.query_view = false;
        let page = state.visible_data().unwrap();
        assert_eq!(page.table, "Customers");
        assert_eq!(page.offset, 200);
        state.set_table_data(data("Customers", 300));
        state.query_view = true;
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
            title: "Sample".into(),
            format: "ACE16".into(),
            tables: vec![],
        });
        assert!(!state.query_ready());
        state.query_view = true;
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
