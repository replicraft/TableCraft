//! Working Design commands use the contextual Ribbon and the same typed intents.
use crate::localization::tr;
use crate::workspace::ribbon::{self, CommandSize, Icon, command, command_menu, group};
use crate::workspace::{Action, State};
use egui::{Popup, Ui, accesskit::HasPopup};

impl State {
    pub(in crate::workspace) fn query_design_context(&self) -> bool {
        self.query_design_request().is_some()
    }

    pub(in crate::workspace) fn query_design_view_menu(
        &mut self,
        ui: &mut Ui,
        actions: &mut Vec<Action>,
    ) {
        let ready = self.query_design_ready();
        let design = self.query_design_visible();
        let results = self.query_design_results_visible();
        let sql = !design && !results;
        let view = command_menu(
            ui,
            if design {
                Icon::ViewDatasheet
            } else {
                Icon::ViewDesign
            },
            &tr(ui, "ribbon-view"),
            ready,
            CommandSize::Large,
        );
        if !view.enabled() {
            Popup::close_id(ui.ctx(), Popup::default_response_id(&view));
        } else {
            Popup::menu(&view).show(|ui| {
                if command(
                    ui,
                    Icon::ViewDatasheet,
                    &tr(ui, "ribbon-datasheet"),
                    !results && self.query_data.is_some(),
                    CommandSize::Small,
                )
                .clicked()
                {
                    self.query_design
                        .session
                        .get_or_insert_with(Default::default)
                        .mode = super::Mode::Results;
                    ui.close();
                }
                if command(
                    ui,
                    Icon::ViewDesign,
                    &tr(ui, "query-design-view"),
                    !design,
                    CommandSize::Small,
                )
                .clicked()
                {
                    if let Some(action) = self.show_query_design() {
                        actions.push(action);
                    }
                    ui.close();
                }
                if command(
                    ui,
                    Icon::QuerySelect,
                    &tr(ui, "query-sql-view"),
                    !sql,
                    CommandSize::Small,
                )
                .clicked()
                {
                    if let Some(action) = self.show_query_design_sql() {
                        actions.push(action);
                    }
                    ui.close();
                }
            });
        }
        let expanded = Popup::is_id_open(ui.ctx(), Popup::default_response_id(&view));
        ui.ctx().accesskit_node_builder(view.id, |node| {
            node.set_has_popup(HasPopup::Menu);
            node.set_expanded(expanded);
        });
    }

    pub(in crate::workspace) fn query_design_ribbon(
        &mut self,
        ui: &mut Ui,
        actions: &mut Vec<Action>,
    ) {
        let ready = self.query_design_ready();
        ribbon::strip(ui, |ui| {
            group(ui, &tr(ui, "ribbon-views"), |ui| {
                self.query_design_view_menu(ui, actions);
            });
            group(ui, &tr(ui, "qbe-results-group"), |ui| {
                let run = if self.query_ready() && ready {
                    Some(Action::Query)
                } else if self.query_design_results_visible()
                    && self
                        .query_design_draft()
                        .is_none_or(|draft| draft.columns.is_empty())
                {
                    // A cached raw-SQL result does not establish an authored
                    // QBE draft. SQL View retains the existing rerun route.
                    None
                } else {
                    self.query_design_run_action()
                };
                if command(
                    ui,
                    Icon::Run,
                    &tr(ui, "query-run"),
                    run.is_some(),
                    CommandSize::Large,
                )
                .clicked()
                    && let Some(action) = run
                {
                    actions.push(action);
                }
            });
            group(ui, &tr(ui, "ribbon-query-type"), |ui| {
                ui.horizontal_top(|ui| {
                    for column in [
                        &[
                            (Icon::QuerySelect, "ribbon-query-select"),
                            (Icon::QueryMakeTable, "ribbon-query-make-table"),
                            (Icon::QueryAppend, "ribbon-query-append"),
                        ][..],
                        &[
                            (Icon::QueryUpdate, "ribbon-query-update"),
                            (Icon::QueryCrosstab, "ribbon-query-crosstab"),
                            (Icon::QueryDelete, "ribbon-query-delete"),
                        ][..],
                        &[
                            (Icon::QueryUnion, "ribbon-query-union"),
                            (Icon::QueryPassThrough, "ribbon-query-pass-through"),
                            (Icon::QueryDefinition, "ribbon-query-definition"),
                        ][..],
                    ] {
                        ui.vertical(|ui| {
                            for (icon, key) in column {
                                ribbon::unavailable_command(
                                    ui,
                                    &self.localizer,
                                    *icon,
                                    key,
                                    CommandSize::Small,
                                );
                            }
                        });
                    }
                });
            });
            group(ui, &tr(ui, "qbe-setup-group"), |ui| {
                ui.horizontal_top(|ui| {
                    if command(
                        ui,
                        Icon::AddTables,
                        &tr(ui, "qbe-add-tables"),
                        ready && self.query_design_visible(),
                        CommandSize::Large,
                    )
                    .clicked()
                    {
                        self.show_query_design_tables();
                    }
                    ui.vertical(|ui| {
                        for (icon, key) in [
                            (Icon::InsertRows, "ribbon-insert-rows"),
                            (Icon::DeleteRows, "ribbon-delete-rows"),
                            (Icon::Builder, "qbe-builder"),
                        ] {
                            ribbon::unavailable_command(
                                ui,
                                &self.localizer,
                                icon,
                                key,
                                CommandSize::Small,
                            );
                        }
                    });
                    ui.vertical(|ui| {
                        for (icon, key) in [
                            (Icon::InsertColumns, "ribbon-insert-columns"),
                            (Icon::DeleteColumns, "ribbon-delete-columns"),
                        ] {
                            ribbon::unavailable_command(
                                ui,
                                &self.localizer,
                                icon,
                                key,
                                CommandSize::Small,
                            );
                        }
                        ui.horizontal(|ui| {
                            ui.add_enabled_ui(false, |ui| {
                                ui.add(Icon::QueryReturn.image(ui, 16.0));
                                let label = tr(ui, "ribbon-return");
                                let value = tr(ui, "ribbon-all");
                                let unavailable = tr(ui, "ribbon-unavailable");
                                let caption = ui.label(&label);
                                let response = egui::ComboBox::from_id_salt("query_return")
                                    .width(70.0)
                                    .selected_text(&value)
                                    .show_ui(ui, |ui| {
                                        ui.label(&value);
                                    })
                                    .response
                                    .labelled_by(caption.id)
                                    .on_disabled_hover_text(&unavailable);
                                ui.ctx().accesskit_node_builder(response.id, |node| {
                                    node.set_label(label.as_str());
                                    node.set_value(value.as_str());
                                    node.set_description(unavailable.as_str());
                                    node.remove_action(egui::accesskit::Action::Click);
                                    node.remove_action(egui::accesskit::Action::Focus);
                                });
                            });
                        });
                    });
                });
            });
            group(ui, &tr(ui, "qbe-show-hide-group"), |ui| {
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        for (icon, key) in [
                            (Icon::Sum, "qbe-totals"),
                            (Icon::Parameters, "qbe-parameters"),
                            (Icon::Properties, "qbe-property-sheet"),
                        ] {
                            ribbon::unavailable_command(
                                ui,
                                &self.localizer,
                                icon,
                                key,
                                CommandSize::Small,
                            );
                        }
                    });
                    ribbon::unavailable_command(
                        ui,
                        &self.localizer,
                        Icon::TableNames,
                        "ribbon-table-names",
                        CommandSize::Small,
                    );
                });
            });
        });
    }
}

#[cfg(test)]
mod tests;
