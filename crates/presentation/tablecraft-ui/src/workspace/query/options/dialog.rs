//! Local resource preferences have no native Access dialog reference.
use super::QueryPoolLimits;
use crate::localization::{message, tr};
use crate::workspace::{Action, State, diagnostics};
use egui::{Context, DragValue, Grid, Id, Modal, Ui, WidgetInfo, WidgetType};

const DIALOG_WIDTH: f32 = 620.0;
const VIEWPORT_MARGIN: f32 = 24.0;
const FOOTER_HEIGHT: f32 = 190.0;
const NUMBER_WIDTH: f32 = 100.0;

impl State {
    pub(in crate::workspace) fn query_run_limits_button(&mut self, ui: &mut Ui) {
        let available = self.query_limits.current.is_some() && !self.busy;
        if ui
            .add_enabled(available, egui::Button::new(tr(ui, "query-limits-title")))
            .on_disabled_hover_text(tr(ui, "ribbon-unavailable"))
            .clicked()
        {
            self.open_query_run_limits();
        }
    }

    pub(in crate::workspace) fn query_run_limits_dialog(
        &mut self,
        context: &Context,
        actions: &mut Vec<Action>,
    ) {
        let Some(mut draft) = self.query_limits.draft else {
            return;
        };
        let width =
            DIALOG_WIDTH.min((context.viewport_rect().width() - VIEWPORT_MARGIN).max(240.0));
        let mut apply = false;
        let mut cancel = false;
        let response = Modal::new(Id::new("read_query_limits")).show(context, |ui| {
            ui.set_width(width);
            ui.heading(tr(ui, "query-limits-title"));
            ui.label(tr(ui, "query-limits-help"));
            ui.separator();
            egui::ScrollArea::both()
                .max_height((context.viewport_rect().height() - FOOTER_HEIGHT).max(80.0))
                .show(ui, |ui| {
                    Grid::new("query_snapshot_limits").show(ui, |ui| {
                        snapshot_field(ui, "query-limits-snapshot", &mut draft.snapshot_rows);
                        snapshot_field(ui, "query-limits-batch", &mut draft.batch_rows);
                    });
                    ui.separator();
                    Grid::new("query_resource_limits")
                        .striped(true)
                        .show(ui, |ui| {
                            for key in [
                                "query-limits-pool",
                                "query-limits-bytes",
                                "query-limits-rows",
                                "query-limits-steps",
                            ] {
                                ui.strong(tr(ui, key));
                            }
                            ui.end_row();
                            pool_row(ui, "query-limits-native", &mut draft.native, true);
                            pool_row(ui, "query-limits-execution", &mut draft.query, true);
                            pool_row(ui, "query-limits-comparison", &mut draft.comparison, false);
                            pool_row(
                                ui,
                                "query-limits-presentation",
                                &mut draft.presentation,
                                false,
                            );
                        });
                });
            if let Some(error) = &self.query_limits.error {
                diagnostics::show(ui, Id::new("query_limits_error"), error);
            }
            ui.separator();
            ui.horizontal(|ui| {
                apply = ui
                    .add_enabled(
                        !self.query_limits.submitted,
                        egui::Button::new(tr(ui, "query-limits-apply")),
                    )
                    .clicked();
                cancel = ui.button(tr(ui, "query-limits-cancel")).clicked();
            });
        });
        if cancel || response.should_close() {
            self.query_limits.draft = None;
            self.query_limits.error = None;
            self.query_limits.submitted = false;
            return;
        }
        self.query_limits.draft = Some(draft);
        if apply {
            self.query_limits.submitted = true;
            actions.push(Action::ConfigureQueryRunLimits { limits: draft });
        }
    }
}

fn snapshot_field(ui: &mut Ui, key: &str, value: &mut u32) {
    let label = tr(ui, key);
    ui.label(&label);
    let response = ui.add_sized(
        [NUMBER_WIDTH, ui.spacing().interact_size.y],
        DragValue::new(value).range(1..=u32::MAX),
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::DragValue, true, label));
    ui.end_row();
}

fn pool_row(ui: &mut Ui, key: &str, pool: &mut QueryPoolLimits, rows: bool) {
    let label = tr(ui, key);
    ui.label(&label);
    for (dimension, value, editable) in [
        ("query-limits-bytes", &mut pool.bytes, true),
        ("query-limits-rows", &mut pool.rows, rows),
        ("query-limits-steps", &mut pool.steps, true),
    ] {
        if editable {
            let accessible = message(
                ui,
                "query-limits-input",
                &[("pool", label.to_owned()), ("dimension", tr(ui, dimension))],
            );
            let response = ui.add_sized(
                [NUMBER_WIDTH, ui.spacing().interact_size.y],
                DragValue::new(value).range(1..=u64::MAX),
            );
            response.widget_info(|| WidgetInfo::labeled(WidgetType::DragValue, true, accessible));
        } else {
            ui.label("—");
        }
    }
    ui.end_row();
}
