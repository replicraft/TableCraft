//! Selection, toggle and clear operate on retained native filter capabilities.
use super::{FilterComparison, Operation};
use crate::CellKind;
use crate::localization::{message, tr};
use crate::workspace::ribbon::{CommandSize, Icon, command, command_menu};
use crate::workspace::{Action, State, diagnostics};
use egui::{Popup, Ui, accesskit::HasPopup};

const FILTER_PAGE_ROWS: u32 = 100;

impl State {
    pub(in crate::workspace) fn filter_commands(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        self.filter_selection_menu(ui, actions);
        self.filter_clear_menu(ui, actions);
        let target = self.active_filter_target();
        let enabled = !self.table_filter_enabled();
        if command(
            ui,
            Icon::ToggleFilter,
            &tr(ui, "ribbon-toggle-filter"),
            target
                .as_ref()
                .is_some_and(|target| self.can_filter_page(target, 0, enabled)),
            CommandSize::Small,
        )
        .on_hover_text(tr(ui, "filtering-toggle-help"))
        .clicked()
            && let Some(target) = target
        {
            actions.push(Action::FilterPage {
                target,
                offset: 0,
                enabled,
            });
        }
    }

    fn filter_selection_menu(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let target = self.current_filter_selection();
        let response = command_menu(
            ui,
            Icon::Selection,
            &tr(ui, "ribbon-selection"),
            target.is_some(),
            CommandSize::Small,
        )
        .on_hover_text(tr(ui, "filtering-selection-help"));
        if !response.enabled() {
            Popup::close_id(ui.ctx(), Popup::default_response_id(&response));
            return;
        }
        let target = target.expect("enabled native selected value");
        let is_null = self.data.as_ref().is_some_and(|data| {
            let Some((row, column)) = self.table_selection.position() else {
                return false;
            };
            data.rows
                .get(row)
                .and_then(|row| row.get(column))
                .is_some_and(|cell| matches!(cell.kind, CellKind::Scalar { is_null: true, .. }))
        });
        Popup::menu(&response).show(|ui| {
            for (comparison, key) in [
                (
                    FilterComparison::Equal,
                    if is_null {
                        "filtering-is-null"
                    } else {
                        "filtering-equal"
                    },
                ),
                (
                    FilterComparison::NotEqual,
                    if is_null {
                        "filtering-is-not-null"
                    } else {
                        "filtering-not-equal"
                    },
                ),
            ] {
                if ui.button(tr(ui, key)).clicked() {
                    actions.push(Action::FilterSelection {
                        // NO_CLONE: Action::FilterSelection needs its own captured identity
                        // after this render/dispatch; keep the original catalog,
                        // input or owner available for the later checks in this
                        // operation.
                        target: target.clone(),
                        comparison,
                    });
                    ui.close();
                }
            }
        });
        popup_accessibility(ui, &response);
    }

    fn filter_clear_menu(&mut self, ui: &mut Ui, actions: &mut Vec<Action>) {
        let target = self.active_filter_target();
        let response = command_menu(
            ui,
            Icon::Advanced,
            &tr(ui, "ribbon-advanced"),
            target
                .as_ref()
                .is_some_and(|target| self.can_clear_filter(target)),
            CommandSize::Small,
        );
        if !response.enabled() {
            Popup::close_id(ui.ctx(), Popup::default_response_id(&response));
            return;
        }
        Popup::menu(&response).show(|ui| {
            if ui.button(tr(ui, "filtering-clear-all")).clicked()
                && let Some(target) = target
            {
                actions.push(Action::ClearTableFilter { target });
                ui.close();
            }
        });
        popup_accessibility(ui, &response);
    }

    pub(in crate::workspace) fn filter_status(&self, ui: &mut Ui) {
        if self.active_filter_target().is_some() {
            let view = self
                .table_filter
                .view
                .as_ref()
                .expect("retained filter view");
            ui.label(message(
                ui,
                if view.filter_count == 0 {
                    "filtering-source-count"
                } else if view.enabled {
                    "filtering-filtered-count"
                } else {
                    "filtering-unfiltered-count"
                },
                &[
                    ("matched", view.matched_rows.to_string()),
                    ("source", view.source_rows.to_string()),
                ],
            ));
            if let Some(sort) = &view.sort {
                ui.label(message(
                    ui,
                    if sort.descending {
                        "filtering-sort-descending"
                    } else {
                        "filtering-sort-ascending"
                    },
                    &[("column", sort.column.as_str())],
                ));
            }
        }
        if self.mock_preview && self.active_filter_target().is_some() {
            ui.small(tr(ui, "filtering-mock-membership"));
        }
        if let Some(pending) = &self.table_filter.pending {
            ui.spinner();
            let key = match pending.operation {
                Operation::Selection {
                    comparison: FilterComparison::Equal,
                    ..
                } => "filtering-applying-equal",
                Operation::Selection {
                    comparison: FilterComparison::NotEqual,
                    ..
                } => "filtering-applying-not-equal",
                Operation::Field { .. } => "filtering-field-applying",
                Operation::Page { .. } | Operation::Restore { .. } => "filtering-loading",
                Operation::Clear { .. } => "filtering-clearing",
                Operation::Sort { .. } => "filtering-sorting",
            };
            ui.label(tr(ui, key));
        }
    }

    pub(in crate::workspace) fn filter_report(&self, ui: &mut Ui) {
        if let Some(error) = self.filter_error_message() {
            diagnostics::show(ui, ui.id().with("filter_error"), error);
        }
    }

    pub(in crate::workspace) fn filtered_page_navigator(
        &self,
        ui: &mut Ui,
        actions: &mut Vec<Action>,
    ) -> bool {
        let Some(target) = self.active_filter_target() else {
            return false;
        };
        let data = self.data.as_ref().expect("retained native filter page");
        let enabled = self.table_filter_enabled();
        let previous = data.offset.saturating_sub(FILTER_PAGE_ROWS);
        if ui
            .add_enabled(
                data.offset > 0 && self.can_filter_page(&target, previous, enabled),
                egui::Button::new(tr(ui, "datasheet-previous-page")),
            )
            .clicked()
        {
            actions.push(Action::FilterPage {
                // NO_CLONE: Action::FilterPage needs its own captured identity after this
                // render/dispatch; keep the original catalog, input or owner
                // available for the later checks in this operation.
                target: target.clone(),
                offset: previous,
                enabled,
            });
        }
        let end = u32::try_from(data.rows.len())
            .ok()
            .and_then(|count| data.offset.checked_add(count));
        ui.label(message(
            ui,
            "datasheet-row-range",
            &[
                (
                    "first",
                    if data.rows.is_empty() {
                        0
                    } else {
                        u64::from(data.offset) + 1
                    }
                    .to_string(),
                ),
                ("last", end.unwrap_or(data.offset).to_string()),
            ],
        ));
        if ui
            .add_enabled(
                data.has_more
                    && end.is_some_and(|offset| self.can_filter_page(&target, offset, enabled)),
                egui::Button::new(tr(ui, "datasheet-next-page")),
            )
            .clicked()
            && let Some(offset) = end
        {
            actions.push(Action::FilterPage {
                target,
                offset,
                enabled,
            });
        }
        true
    }
}

fn popup_accessibility(ui: &Ui, response: &egui::Response) {
    let expanded = Popup::is_id_open(ui.ctx(), Popup::default_response_id(response));
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_has_popup(HasPopup::Menu);
        node.set_expanded(expanded);
    });
}
