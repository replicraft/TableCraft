//! Working commands and visible unavailable controls share the native Ribbon layout.
pub(super) mod menus;
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
        if ribbon::tab(ui, &t, self.ribbon_tab) {
            return;
        }
        ribbon::strip(ui, |ui| {
            group(ui, &t.text("ribbon-views"), |ui| {
                self.view_menu(ui, actions);
            });
            if self.ribbon_tab == ribbon::Tab::TableDesign {
                ribbon::design(ui, &t);
                return;
            }
            ribbon::group_with_launcher(ui, &t.text("ribbon-clipboard"), true, |ui| {
                ui.horizontal(|ui| {
                    self.paste_menu(ui);
                    ui.vertical(|ui| {
                        ribbon::unavailable_command(
                            ui,
                            &t,
                            Icon::Cut,
                            "ribbon-cut",
                            CommandSize::Small,
                        );
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
                        ribbon::unavailable_command(
                            ui,
                            &t,
                            Icon::PaintBrush,
                            "ribbon-format-painter",
                            CommandSize::Small,
                        );
                    });
                });
            });
            self.read_commands(ui, actions);
            self.find_commands(ui, actions);
            ribbon::home(ui, &t, self.font_families.families());
            group(ui, &t.text("ribbon-zoom"), |ui| self.zoom_menu(ui));
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
                    self.select_menu(ui);
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
                let refresh = page.map(|data| Action::Page {
                    table: data.table.clone(),
                    offset: data.offset,
                });
                self.refresh_menu(ui, refresh, actions);
                ui.vertical(|ui| {
                    let target = self.current_insert_target();
                    if command(
                        ui,
                        Icon::Add,
                        &t.text("ribbon-new"),
                        target.is_some(),
                        CommandSize::Small,
                    )
                    .on_hover_text(crate::localization::message(
                        ui,
                        "insertion-shortcut-help",
                        &[(
                            "shortcut",
                            crate::keyboard::new_record_shortcut_text(ui.ctx()),
                        )],
                    ))
                    .on_hover_text(t.text("insertion-intent-help"))
                    .clicked()
                        && let Some(target) = target
                    {
                        actions.push(Action::BeginInsert { target });
                    }
                    if command(
                        ui,
                        Icon::Save,
                        &t.text("ribbon-save"),
                        self.can_save_table_draft(),
                        CommandSize::Small,
                    )
                    .on_disabled_hover_text(t.text("table-draft-save-unavailable"))
                    .clicked()
                    {
                        self.request_table_draft_save();
                    }
                    self.delete_menu(ui, actions);
                });
                ui.vertical(|ui| {
                    self.totals_command(ui, actions);
                    for (icon, key) in [
                        (Icon::Spelling, "ribbon-spelling"),
                        (Icon::MoreRecords, "ribbon-more"),
                    ] {
                        ribbon::unavailable_command(ui, &t, icon, key, CommandSize::Small);
                    }
                });
            });
        });
    }
}

#[cfg(test)]
mod tests;
