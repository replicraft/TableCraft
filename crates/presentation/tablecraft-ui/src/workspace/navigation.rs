//! Object navigation emits page intents without owning a database source.
use super::{Action, State};
use crate::theme::ACCENT;
use egui::{RichText, ScrollArea, TextEdit, Ui};

impl State {
    pub(super) fn navigation(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
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
    }
}
