//! Working commands share the dense ribbon with explicitly disabled previews.
mod view;

use super::{Action, DocumentView, State, ribbon};
use ribbon::{CommandSize, Icon, command, group};

impl State {
    pub fn set_sorted_query_data(&mut self, data: super::Data) -> bool {
        if data.source_revision.is_some() || data.offset != 0 || data.has_more {
            return false;
        }
        self.set_query_data(data);
        self.sorted_query = true;
        self.saved_query.active = false;
        self.select_view(DocumentView::Query);
        true
    }

    pub(super) fn home(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let t = self.localizer.clone();
        ribbon::strip(ui, |ui| {
            group(ui, &t.text("ribbon-views"), |ui| {
                self.view_menu(ui, actions);
            });
            if self.ribbon_tab == ribbon::Tab::TableDesign && self.mock_preview {
                ribbon::design(ui, &t);
                return;
            }
            group(ui, &t.text("ribbon-clipboard"), |ui| {
                ui.horizontal(|ui| {
                    if self.mock_preview {
                        ribbon::mock_command(
                            ui,
                            &t,
                            Icon::Paste,
                            "ribbon-paste",
                            CommandSize::Large,
                        );
                    }
                    ui.vertical(|ui| {
                        if self.mock_preview {
                            ribbon::mock_command(
                                ui,
                                &t,
                                Icon::Cut,
                                "ribbon-cut",
                                CommandSize::Small,
                            );
                        }
                        if command(
                            ui,
                            Icon::Copy,
                            &t.text("ribbon-copy"),
                            self.can_copy_selection(),
                            CommandSize::Small,
                        )
                        .on_hover_text(t.text(if self.record_selected() {
                            "ribbon-copy-record-help"
                        } else {
                            "ribbon-copy-cell-help"
                        }))
                        .clicked()
                        {
                            self.copy_selected(ui.ctx());
                        }
                        if self.mock_preview {
                            ribbon::mock_command(
                                ui,
                                &t,
                                Icon::PaintBrush,
                                "ribbon-format-painter",
                                CommandSize::Small,
                            );
                        }
                    });
                });
            });
            self.read_commands(ui, actions);
            self.find_commands(ui, actions);
            if self.mock_preview {
                ribbon::home(ui, &t);
            }
        });
    }

    fn find_commands(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let t = self.localizer.clone();
        group(ui, &t.text("ribbon-find-group"), |ui| {
            ui.horizontal(|ui| {
                if self.mock_preview {
                    ribbon::mock_command(ui, &t, Icon::Search, "ribbon-find", CommandSize::Large);
                }
                ui.vertical(|ui| {
                    if self.mock_preview {
                        ribbon::mock_command(
                            ui,
                            &t,
                            Icon::Replace,
                            "ribbon-replace",
                            CommandSize::Small,
                        );
                    }
                    self.record_menu(ui, actions);
                    if command(
                        ui,
                        Icon::Cursor,
                        &t.text("ribbon-select-record"),
                        self.can_select_record(),
                        CommandSize::Small,
                    )
                    .clicked()
                    {
                        self.select_current_record(ui.ctx());
                    }
                });
            });
        });
    }

    pub fn can_sort_table(&self, document_revision: u64, table: &str, column: &str) -> bool {
        self.view == DocumentView::Table
            && !self.busy
            && !self.backstage_visible()
            && !self.edit_dialog_open()
            && self.contents_request_id().is_none()
            && !self.saved_query_prompt_open()
            && self
                .database
                .as_ref()
                .is_some_and(|database| database.document_revision == document_revision)
            && self.data.as_ref().is_some_and(|data| {
                data.source_revision == Some(document_revision)
                    && data.table == table
                    && data.columns.iter().any(|field| field.name == column)
            })
    }

    fn selected_sort(&self, descending: bool) -> Option<Action> {
        let data = self.data.as_ref()?;
        let document_revision = data.source_revision?;
        let column = &data.columns.get(self.table_selection.position()?.1)?.name;
        self.can_sort_table(document_revision, &data.table, column)
            .then(|| Action::SortTable {
                document_revision,
                table: data.table.clone(),
                column: column.clone(),
                descending,
            })
    }

    fn read_commands(&self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let t = self.localizer.clone();
        group(ui, &t.text("ribbon-sort-filter"), |ui| {
            ui.horizontal(|ui| {
                if self.mock_preview {
                    ribbon::mock_command(ui, &t, Icon::Filter, "ribbon-filter", CommandSize::Large);
                }
                ui.vertical(|ui| {
                    for (descending, icon, key) in [
                        (false, Icon::Ascending, "ribbon-ascending"),
                        (true, Icon::Descending, "ribbon-descending"),
                    ] {
                        let action = self.selected_sort(descending);
                        if command(ui, icon, &t.text(key), action.is_some(), CommandSize::Small)
                            .on_hover_text(t.text("ribbon-sort-query-help"))
                            .clicked()
                            && let Some(action) = action
                        {
                            actions.push(action);
                        }
                    }
                    if self.mock_preview {
                        ribbon::mock_command(
                            ui,
                            &t,
                            Icon::RemoveSort,
                            "ribbon-remove-sort",
                            CommandSize::Small,
                        );
                    }
                });
                if self.mock_preview {
                    ui.vertical(|ui| {
                        for (icon, key) in [
                            (Icon::Selection, "ribbon-selection"),
                            (Icon::Advanced, "ribbon-advanced"),
                            (Icon::ToggleFilter, "ribbon-toggle-filter"),
                        ] {
                            ribbon::mock_command(ui, &t, icon, key, CommandSize::Small);
                        }
                    });
                }
            });
        });
        group(ui, &t.text("ribbon-records"), |ui| {
            ui.horizontal(|ui| {
                let page = self.data.as_ref().filter(|_| {
                    self.view == DocumentView::Table
                        && !self.busy
                        && !self.edit_dialog_open()
                        && self.contents_request_id().is_none()
                        && !self.saved_query_prompt_open()
                });
                if command(
                    ui,
                    Icon::Refresh,
                    &t.text("ribbon-refresh-table"),
                    page.is_some(),
                    CommandSize::Large,
                )
                .on_hover_text(t.text("ribbon-refresh-table-help"))
                .clicked()
                    && let Some(data) = page
                {
                    actions.push(Action::Page {
                        table: data.table.clone(),
                        offset: data.offset,
                    });
                }
                ui.vertical(|ui| {
                    let target = self.current_insert_target();
                    if command(
                        ui,
                        Icon::Add,
                        &t.text("insertion-title"),
                        target.is_some(),
                        CommandSize::Small,
                    )
                    .clicked()
                        && let Some(target) = target
                    {
                        actions.push(Action::BeginInsert { target });
                    }
                    let target = self.current_delete_target();
                    if command(
                        ui,
                        Icon::Delete,
                        &t.text("deletion-title"),
                        target.is_some(),
                        CommandSize::Small,
                    )
                    .clicked()
                        && let Some(target) = target
                    {
                        actions.push(Action::ReviewDelete { target });
                    }
                    if self.mock_preview {
                        ribbon::mock_command(ui, &t, Icon::Save, "ribbon-save", CommandSize::Small);
                    }
                });
                if self.mock_preview {
                    ui.vertical(|ui| {
                        for (icon, key) in [
                            (Icon::Sum, "ribbon-totals"),
                            (Icon::Spelling, "ribbon-spelling"),
                            (Icon::MoreRecords, "ribbon-more"),
                        ] {
                            ribbon::mock_command(ui, &t, icon, key, CommandSize::Small);
                        }
                    });
                }
            });
        });
    }
}

#[cfg(test)]
mod tests;
