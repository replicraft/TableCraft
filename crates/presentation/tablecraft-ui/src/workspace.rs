use super::{
    datasheet,
    theme::{ACCENT, MUTED},
};
use egui::{Color32, Frame, Margin, RichText, ScrollArea, TextEdit, Ui};

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
    pub database: Option<Database>,
    pub data: Option<Data>,
    pub busy: bool,
    pub error: Option<String>,
    pub sql: String,
    filter: String,
    query_view: bool,
    show_system: bool,
    about: bool,
}

impl State {
    /// Draw the workspace and return explicit user actions; performs no I/O.
    pub fn show(&mut self, ui: &mut Ui) -> Vec<Action> {
        let mut actions = Vec::new();
        egui::Panel::top("title")
            .frame(
                Frame::new()
                    .fill(ACCENT)
                    .inner_margin(Margin::symmetric(18, 10)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("TABLECRAFT")
                            .color(Color32::WHITE)
                            .strong()
                            .size(17.0),
                    );
                    ui.separator();
                    ui.label(
                        RichText::new(
                            self.database
                                .as_ref()
                                .map_or("Database workspace", |db| db.title.as_str()),
                        )
                        .color(Color32::WHITE),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(RichText::new("About").color(Color32::WHITE))
                            .clicked()
                        {
                            self.about = true;
                        }
                        ui.label(
                            RichText::new("READ ONLY · PREVIEW")
                                .color(Color32::WHITE)
                                .size(11.0),
                        );
                    });
                });
            });
        egui::Panel::top("ribbon").show(ui, |ui| {
            ui.add_space(3.0);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.query_view, false, "Home");
                ui.selectable_value(&mut self.query_view, true, "Query");
            });
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Database");
                ui.add_sized(
                    [(ui.available_width().min(530.0) - 190.0).max(150.0), 28.0],
                    TextEdit::singleline(&mut self.path).hint_text("Path to .accdb or .mdb"),
                );
                ui.add_sized(
                    [130.0, 28.0],
                    TextEdit::singleline(&mut self.password)
                        .password(true)
                        .hint_text("Password"),
                );
                if ui
                    .add_enabled(
                        !self.path.trim().is_empty(),
                        egui::Button::new("Open database"),
                    )
                    .clicked()
                {
                    actions.push(Action::Open);
                }
                if self.busy {
                    ui.spinner();
                    if ui.button("Cancel").clicked() {
                        actions.push(Action::Cancel);
                    }
                }
            });
            ui.small("Open a stable copy or close other database writers before opening.");
            ui.add_space(4.0);
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.small(if self.busy { "Working…" } else { "Ready" });
                ui.separator();
                ui.small(
                    self.database
                        .as_ref()
                        .map_or("Local Access databases", |db| db.format.as_str()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.small("Snapshot browsing · original file unchanged");
                });
            });
        });
        egui::Panel::left("navigation")
            .default_size(245.0)
            .resizable(true)
            .show(ui, |ui| {
                ui.add_space(10.0);
                ui.label(RichText::new("All Access Objects").strong().size(17.0));
                ui.add(TextEdit::singleline(&mut self.filter).hint_text("Search tables"));
                ui.checkbox(&mut self.show_system, "Show system tables");
                ui.separator();
                if let Some(db) = &self.database {
                    let filter = self.filter.to_lowercase();
                    ScrollArea::vertical().show(ui, |ui| {
                        for table in &db.tables {
                            if (!self.show_system && table.system)
                                || !table.name.to_lowercase().contains(&filter)
                            {
                                continue;
                            }
                            let selected = self
                                .data
                                .as_ref()
                                .is_some_and(|data| data.table == table.name)
                                && !self.query_view;
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("▦").color(ACCENT));
                                if ui
                                    .selectable_label(selected, &table.name)
                                    .on_hover_text(&table.storage)
                                    .clicked()
                                {
                                    self.query_view = false;
                                    actions.push(Action::Page {
                                        table: table.name.clone(),
                                        offset: 0,
                                    });
                                }
                                if table.system {
                                    ui.weak("system");
                                }
                            });
                        }
                    });
                } else {
                    ui.weak("Open a database to see its tables.");
                }
            });
        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(error) = &self.error {
                Frame::new()
                    .fill(Color32::from_rgb(255, 237, 237))
                    .inner_margin(12)
                    .show(ui, |ui| {
                        ui.colored_label(ACCENT, error);
                    });
                ui.add_space(10.0);
            }
            if self.query_view {
                ui.heading("SQL query");
                ui.label(
                    RichText::new("Run a supported SELECT against the opened snapshot.")
                        .color(MUTED),
                );
                ui.add(
                    TextEdit::multiline(&mut self.sql)
                        .code_editor()
                        .desired_rows(5)
                        .desired_width(f32::INFINITY)
                        .hint_text("SELECT TOP 100 * FROM [Customers]"),
                );
                if ui
                    .add_enabled(
                        self.database.is_some() && !self.busy && !self.sql.trim().is_empty(),
                        egui::Button::new("Run query"),
                    )
                    .clicked()
                {
                    actions.push(Action::Query);
                }
                ui.separator();
            }
            if let Some(data) = &self.data {
                ui.horizontal(|ui| {
                    ui.heading(&data.table);
                    ui.label(RichText::new(format!("{} columns", data.columns.len())).color(MUTED));
                });
                if !self.query_view {
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(
                                !self.busy && data.offset > 0,
                                egui::Button::new("‹ Previous"),
                            )
                            .clicked()
                        {
                            actions.push(Action::Page {
                                table: data.table.clone(),
                                offset: data.offset.saturating_sub(100),
                            });
                        }
                        ui.label(format!(
                            "Rows {}–{}",
                            if data.rows.is_empty() {
                                0
                            } else {
                                u64::from(data.offset) + 1
                            },
                            u64::from(data.offset) + data.rows.len() as u64
                        ));
                        if ui
                            .add_enabled(!self.busy && data.has_more, egui::Button::new("Next ›"))
                            .clicked()
                        {
                            actions.push(Action::Page {
                                table: data.table.clone(),
                                offset: data.offset.saturating_add(100),
                            });
                        }
                    });
                }
                ui.separator();
                if data.rows.is_empty() {
                    ui.weak("No rows.");
                } else {
                    datasheet::show(ui, data);
                }
            } else {
                ui.add_space(80.0);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Your data. A familiar workspace.").size(29.0));
                    ui.add_space(12.0);
                    ui.label(
                        "Open an Access database, then choose a table to browse its contents.",
                    );
                    ui.add_space(8.0);
                    ui.weak("MDB and ACCDB · tables and supported SELECT queries");
                });
            }
        });
        egui::Window::new("About TableCraft")
            .open(&mut self.about)
            .show(ui.ctx(), |ui| {
                ui.heading("TableCraft");
                ui.label("A cross-platform Access workspace. Desktop preview.");
                ui.label("Selawik regular · © 2015 Microsoft Corporation");
                ui.hyperlink_to(
                    "Font source: Microsoft Selawik 1.01",
                    "https://github.com/microsoft/Selawik/releases/tag/1.01",
                );
                egui::CollapsingHeader::new("Selawik license").show(ui, |ui| {
                    ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
                        ui.label(include_str!("../../../../assets/ui/fonts/OFL-Selawik.txt"));
                    });
                });
            });
        actions
    }
}
