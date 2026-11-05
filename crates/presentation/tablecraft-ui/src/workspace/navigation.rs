//! Object navigation emits page intents without owning a database source.
mod row;

use super::focus::{self, Pane};
use super::{Action, DocumentView, SavedQueryTarget, State, ribbon::Icon};
use crate::localization::tr;
use egui::{RichText, ScrollArea, TextEdit, Ui};

const SEARCH_HEIGHT: f32 = 28.0;
const SEARCH_ICON_SIZE: f32 = 16.0;
const SEARCH_PADDING: f32 = 6.0;
const SEARCH_VERTICAL_PADDING: i8 = 5;

impl State {
    pub(super) fn navigation(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let mut selected_table = false;
        egui::Panel::left("navigation")
            .default_size(210.0)
            .resizable(true)
            .show(ui, |ui| {
                ui.add_space(5.0);
                let heading = tr(ui, "navigation-all-objects");
                ui.add(egui::Label::new(RichText::new(&heading).strong().size(15.0)).truncate())
                    .on_hover_text(heading);
                let search_label = tr(ui, "navigation-search-objects");
                let search = ui.add_sized(
                    [ui.available_width(), SEARCH_HEIGHT],
                    TextEdit::singleline(&mut self.filter)
                        .hint_text(&search_label)
                        .margin(egui::Margin {
                            left: SEARCH_PADDING as i8,
                            right: (SEARCH_ICON_SIZE + SEARCH_PADDING * 2.0) as i8,
                            top: SEARCH_VERTICAL_PADDING,
                            bottom: SEARCH_VERTICAL_PADDING,
                        }),
                );
                let icon_rect = egui::Rect::from_center_size(
                    egui::pos2(
                        search.rect.right() - SEARCH_PADDING - SEARCH_ICON_SIZE * 0.5,
                        search.rect.center().y,
                    ),
                    egui::Vec2::splat(SEARCH_ICON_SIZE),
                );
                Icon::Search
                    .image(ui, SEARCH_ICON_SIZE)
                    .paint_at(ui, icon_rect);
                crate::theme::input_border(ui, &search);
                focus::register(ui, Pane::Navigation, &search, false);
                ui.ctx().accesskit_node_builder(search.id, |node| {
                    node.set_label(search_label);
                });
                let system = ui.checkbox(&mut self.show_system, tr(ui, "navigation-show-system"));
                focus::register(ui, Pane::Navigation, &system, false);
                ui.separator();
                if let Some(db) = &self.database {
                    let filter = self.filter.to_lowercase();
                    let system_label = tr(ui, "navigation-system");
                    ScrollArea::vertical()
                        .id_salt(("navigation_objects", db.document_revision))
                        .show(ui, |ui| {
                            ui.label(RichText::new(tr(ui, "navigation-tables")).strong());
                            for table in &db.tables {
                                if (!self.show_system && table.system)
                                    || !table.name.to_lowercase().contains(&filter)
                                {
                                    continue;
                                }
                                let selected = match self.view {
                                    DocumentView::Table => self
                                        .data
                                        .as_ref()
                                        .is_some_and(|data| data.table == table.name),
                                    DocumentView::Design => self
                                        .schema
                                        .as_ref()
                                        .is_some_and(|schema| schema.table == table.name),
                                    DocumentView::Query => false,
                                };
                                ui.push_id(("table", db.document_revision, &table.name), |ui| {
                                    let response = row::show(
                                        ui,
                                        Icon::Table,
                                        &table.name,
                                        selected,
                                        table.system.then_some(system_label.as_str()),
                                    )
                                    .on_hover_text(&table.storage);
                                    focus::register(ui, Pane::Navigation, &response, selected);
                                    response.context_menu(|ui| {
                                        if ui
                                            .add_enabled(
                                                !self.busy,
                                                egui::Button::new(tr(ui, "navigation-view-schema")),
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
                                });
                            }
                            ui.separator();
                            ui.label(RichText::new(tr(ui, "navigation-queries")).strong());
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
                                ui.push_id(
                                    ("query", db.document_revision, query.object_id),
                                    |ui| {
                                        ui.add_enabled_ui(!self.busy, |ui| {
                                            let response = row::show(
                                                ui,
                                                Icon::Search,
                                                &query.name,
                                                selected,
                                                query.system.then_some(system_label.as_str()),
                                            );
                                            focus::register(
                                                ui,
                                                Pane::Navigation,
                                                &response,
                                                selected,
                                            );
                                            if response.clicked() {
                                                actions.push(Action::InspectSavedQuery {
                                                    target: SavedQueryTarget {
                                                        document_revision: db.document_revision,
                                                        object_id: query.object_id,
                                                        name: query.name.clone(),
                                                    },
                                                });
                                            }
                                        });
                                    },
                                );
                            }
                        });
                } else {
                    ui.weak(tr(ui, "navigation-open-first"));
                }
            });
        if selected_table {
            self.select_view(DocumentView::Table);
        }
    }
}

#[cfg(test)]
mod tests;
