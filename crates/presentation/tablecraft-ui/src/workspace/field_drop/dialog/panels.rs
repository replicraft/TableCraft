use crate::{
    Action, State,
    localization::{message, tr},
    workspace::{diagnostics, save_display},
};
use egui::Ui;

const PENDING_HEIGHT: f32 = 100.0;
const WARNING_HEIGHT: f32 = 270.0;
const MAX_DOCUMENT_FRACTION: f32 = 0.5;
const PADDING: f32 = 8.0;

impl State {
    /// Reserve a bounded bottom panel before Datasheet/Design consumes the pane.
    pub(in crate::workspace) fn field_drop_source_panel(
        &mut self,
        ui: &mut Ui,
        _actions: &mut Vec<Action>,
    ) {
        if self.field_drop.status.is_some()
            || (!self.field_drop_dirty() && !self.field_drop_source_stale())
        {
            return;
        }
        let warning = self.field_drop_source_stale();
        let details = self
            .field_drop
            .warning
            .as_ref()
            .is_some_and(|warning| warning.open);
        let requested = if warning && details {
            WARNING_HEIGHT
        } else {
            PENDING_HEIGHT
        };
        let height = requested.min(ui.available_height() * MAX_DOCUMENT_FRACTION);
        egui::Panel::bottom(ui.make_persistent_id("field_drop_source_panel"))
            .resizable(false)
            .exact_size(height)
            .frame(egui::Frame::new().inner_margin(PADDING))
            .show(ui, |ui| {
                if warning {
                    self.field_drop_warning_panel(ui);
                } else {
                    self.field_drop_pending_panel(ui);
                }
            });
    }

    fn field_drop_pending_panel(&mut self, ui: &mut Ui) {
        let Some((target, _)) = self.pending_field_drop() else {
            return;
        };
        let can_save = self.can_review_field_drop();
        let footer = ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
        egui::ScrollArea::vertical()
            .id_salt("field_drop_pending_details")
            .max_height((ui.available_height() - footer).max(0.0))
            .show(ui, |ui| {
                ui.label(message(
                    ui,
                    "field-drop-pending",
                    &[("column", target.column.as_str())],
                ));
                ui.small(tr(ui, "field-drop-pending-help"));
            });
        let mut save = false;
        let mut close = false;
        ui.horizontal_wrapped(|ui| {
            save = ui
                .add_enabled(can_save, egui::Button::new(tr(ui, "field-drop-close-save")))
                .clicked();
            close = ui.button(tr(ui, "backstage-close")).clicked();
        });
        if save {
            self.queue_active_save();
        }
        if close {
            self.request_current_field_drop_close();
        }
    }

    fn field_drop_warning_panel(&mut self, ui: &mut Ui) {
        let Some(warning) = self.field_drop_warning() else {
            return;
        };
        let details = self
            .field_drop
            .warning
            .as_ref()
            .is_some_and(|warning| warning.open);
        let footer = if details {
            ui.spacing().interact_size.y + ui.spacing().item_spacing.y
        } else {
            0.0
        };
        egui::ScrollArea::vertical()
            .id_salt("field_drop_warning_details")
            .max_height((ui.available_height() - footer).max(0.0))
            .show(ui, |ui| {
                ui.label(tr(ui, "field-drop-warning-saved"));
                if details {
                    if let Some(outcome) = warning.outcome {
                        save_display::outcome(ui, outcome);
                    }
                    if let Some(error) = warning.reopen_error {
                        diagnostics::show(ui, ui.id().with("drop_warning_error"), error);
                    }
                    if warning.report_omitted {
                        ui.label(tr(ui, "objects-memory-limit"));
                    }
                }
                if warning.cleanup == Some(crate::SaveCleanupNotice::RetainedPrivateJournal) {
                    ui.label(tr(ui, "editing-save-retained-private-journal"));
                }
            });
        if details && ui.button(tr(ui, "field-drop-close-editor")).clicked() {
            self.dismiss_field_drop_warning_details();
        }
    }
}
