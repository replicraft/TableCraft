//! Object navigation emits page intents without owning a database source.
use super::{Action, DocumentView, SavedQueryTarget, State, ribbon::Icon};
use crate::localization::tr;
use egui::{RichText, ScrollArea, TextEdit, Ui};

impl State {
    pub(super) fn navigation(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let mut selected_table = false;
        egui::Panel::left("navigation")
            .default_size(210.0)
            .resizable(true)
            .show(ui, |ui| {
                ui.add_space(5.0);
                ui.label(
                    RichText::new(tr(ui, "datasheet-all-objects"))
                        .strong()
                        .size(15.0),
                );
                ui.add(
                    TextEdit::singleline(&mut self.filter)
                        .hint_text(tr(ui, "datasheet-search-objects")),
                );
                ui.checkbox(&mut self.show_system, tr(ui, "datasheet-show-system"));
                ui.separator();
                if let Some(db) = &self.database {
                    let filter = self.filter.to_lowercase();
                    ScrollArea::vertical().show(ui, |ui| {
                        ui.label(RichText::new(tr(ui, "datasheet-tables")).strong());
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
                                && self.view == DocumentView::Table;
                            ui.horizontal(|ui| {
                                table_icon(ui);
                                let response = ui
                                    .selectable_label(selected, &table.name)
                                    .on_hover_text(&table.storage);
                                response.context_menu(|ui| {
                                    if ui
                                        .add_enabled(
                                            !self.busy,
                                            egui::Button::new(tr(ui, "datasheet-view-schema")),
                                        )
                                        .clicked()
                                    {
                                        actions.push(Action::ViewSchema {
                                            table: table.name.clone(),
                                        });
                                        ui.close();
                                    }
                                });
                                if response.clicked() {
                                    selected_table = true;
                                    actions.push(Action::Page {
                                        table: table.name.clone(),
                                        offset: 0,
                                    });
                                }
                                if table.system {
                                    ui.weak(tr(ui, "datasheet-system"));
                                }
                            });
                        }
                        ui.separator();
                        ui.label(RichText::new(tr(ui, "datasheet-queries")).strong());
                        for query in &db.queries {
                            if (!self.show_system && query.system)
                                || !query.name.to_lowercase().contains(&filter)
                            {
                                continue;
                            }
                            let selected = self.saved_query_visible()
                                && self.saved_query.target.as_ref().is_some_and(|target| {
                                    target.document_revision == db.document_revision
                                        && target.object_id == query.object_id
                                });
                            if ui
                                .add_enabled(
                                    !self.busy,
                                    egui::Button::selectable(selected, &query.name),
                                )
                                .clicked()
                            {
                                actions.push(Action::InspectSavedQuery {
                                    target: SavedQueryTarget {
                                        document_revision: db.document_revision,
                                        object_id: query.object_id,
                                        name: query.name.clone(),
                                    },
                                });
                            }
                        }
                    });
                } else {
                    ui.weak(tr(ui, "datasheet-open-first"));
                }
            });
        if selected_table {
            self.select_view(DocumentView::Table);
        }
    }
}

// The adjacent table-name control supplies the accessible name and action.
fn table_icon(ui: &mut Ui) {
    ui.add(Icon::Table.image(crate::theme::palette(ui.ctx()).ink, 16.0));
}
