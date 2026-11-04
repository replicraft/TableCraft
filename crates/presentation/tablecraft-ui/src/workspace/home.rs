//! Working read-only Home commands, grouped beside the File tab.
use super::{Action, DocumentView, State};
use crate::theme::MUTED;
use egui::{Button, RichText, Ui};

impl State {
    pub(super) fn home(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            ui.vertical(|ui| {
                ui.add_enabled_ui(!self.busy, |ui| {
                    let datasheet_available = match self.view {
                        DocumentView::Table => self.data.is_some(),
                        DocumentView::Design => self.schema.as_ref().is_some_and(|schema| {
                            self.data
                                .as_ref()
                                .is_some_and(|data| data.table == schema.table)
                        }),
                        DocumentView::Query => false,
                    };
                    if ui
                        .add_enabled(datasheet_available, Button::new("Datasheet"))
                        .clicked()
                    {
                        self.select_view(DocumentView::Table);
                    }
                    let table = match self.view {
                        DocumentView::Table => self.data.as_ref().map(|data| data.table.as_str()),
                        DocumentView::Design => {
                            self.schema.as_ref().map(|schema| schema.table.as_str())
                        }
                        DocumentView::Query => None,
                    };
                    if ui
                        .add_enabled(table.is_some(), Button::new("Table Design"))
                        .clicked()
                    {
                        actions.push(Action::ViewSchema {
                            table: table.expect("enabled for an existing table").to_owned(),
                        });
                    }
                });
                ui.label(RichText::new("Views").small().color(MUTED));
            });
            ui.separator();
            ui.vertical(|ui| {
                if ui
                    .add_enabled(self.selected_cell_text().is_some(), Button::new("Copy"))
                    .clicked()
                {
                    self.copy_selected(ui.ctx());
                }
                ui.add_space(ui.spacing().interact_size.y + ui.spacing().item_spacing.y);
                ui.label(RichText::new("Clipboard").small().color(MUTED));
            });
        });
    }
}

#[cfg(test)]
mod tests;
