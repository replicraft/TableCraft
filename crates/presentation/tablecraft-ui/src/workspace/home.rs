//! Working commands share the dense ribbon with explicitly disabled previews.
mod view;

use super::{Action, DocumentView, State, ribbon};
use ribbon::{CommandSize, Icon, command, group};

impl State {
    pub(super) fn home(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        if self.ribbon_tab == ribbon::Tab::QueryDesign && self.query_design_context() {
            self.query_design_ribbon(ui, actions);
            return;
        }
        let t = self.localizer.clone();
        if self.mock_preview && ribbon::mock_tab(ui, &t, self.ribbon_tab) {
            return;
        }
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
                    if command(
                        ui,
                        Icon::Paste,
                        &t.text("editing-paste-text"),
                        self.current_paste_target().is_some(),
                        CommandSize::Large,
                    )
                    .on_hover_text(t.text("editing-paste-help"))
                    .clicked()
                    {
                        self.request_paste(ui.ctx());
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
            group(ui, &t.text("ribbon-zoom"), |ui| {
                if command(
                    ui,
                    Icon::Zoom,
                    &t.text("zoom-title"),
                    self.can_zoom_cell(),
                    CommandSize::Large,
                )
                .on_hover_text(t.text("zoom-read-only"))
                .clicked()
                {
                    self.open_zoom_cell();
                }
            });
        });
    }

    fn find_commands(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let t = self.localizer.clone();
        group(ui, &t.text("ribbon-find-group"), |ui| {
            ui.horizontal(|ui| {
                if command(
                    ui,
                    Icon::Search,
                    &t.text("ribbon-find"),
                    self.can_open_find(),
                    CommandSize::Large,
                )
                .on_disabled_hover_text(t.text(if self.view == DocumentView::Query {
                    "find-query-unavailable"
                } else {
                    "find-unavailable"
                }))
                .clicked()
                {
                    self.open_find();
                }
                ui.vertical(|ui| {
                    if command(
                        ui,
                        Icon::Replace,
                        &t.text("ribbon-replace"),
                        self.can_open_find() && (self.replace_available || self.mock_preview),
                        CommandSize::Small,
                    )
                    .on_disabled_hover_text(t.text("find-replace-unavailable"))
                    .clicked()
                    {
                        self.open_replace();
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

    fn read_commands(&mut self, ui: &mut egui::Ui, actions: &mut Vec<Action>) {
        let t = self.localizer.clone();
        group(ui, &t.text("ribbon-sort-filter"), |ui| {
            ui.horizontal(|ui| {
                let target = self.current_field_filter_target();
                if command(
                    ui,
                    Icon::Filter,
                    &t.text("ribbon-filter"),
                    target.is_some(),
                    CommandSize::Large,
                )
                .on_hover_text(t.text("filtering-field-help"))
                .clicked()
                    && let Some(target) = target
                {
                    actions.push(Action::DescribeFieldFilter { target });
                }
                ui.vertical(|ui| {
                    self.sort_commands(ui, actions);
                });
                ui.vertical(|ui| self.filter_commands(ui, actions));
            });
        });
        group(ui, &t.text("ribbon-records"), |ui| {
            ui.horizontal(|ui| {
                let page = self.data.as_ref().filter(|data| {
                    (data.row_ordinals.is_none() || self.active_filter_target().is_some())
                        && self.view == DocumentView::Table
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
                ui.vertical(|ui| {
                    self.totals_command(ui, actions);
                    if self.mock_preview {
                        for (icon, key) in [
                            (Icon::Spelling, "ribbon-spelling"),
                            (Icon::MoreRecords, "ribbon-more"),
                        ] {
                            ribbon::mock_command(ui, &t, icon, key, CommandSize::Small);
                        }
                    }
                });
            });
        });
    }
}

#[cfg(test)]
mod tests;
