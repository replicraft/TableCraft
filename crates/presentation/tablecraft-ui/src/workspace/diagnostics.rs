//! Bounded, literal service reports; displaying a failure never retries it.
use crate::localization::tr;
use crate::theme::palette;
use egui::{Id, Label, ScrollArea, Sense, Stroke, StrokeKind, Ui};

const MAX_REPORT_HEIGHT: f32 = 96.0;

pub(super) fn show(ui: &mut Ui, id: Id, report: &str) {
    ui.push_id(id, |ui| {
        let copy = ui
            .small_button(tr(ui, "diagnostics-copy"))
            .on_hover_text(tr(ui, "diagnostics-copy-help"));
        if copy.clicked() {
            ui.ctx().copy_text(report.to_owned());
        }
        ui.scope(|ui| {
            ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
            let body = ScrollArea::vertical()
                // A new literal report starts at its beginning. The same report
                // retains its viewport through redraws, theme and UI language.
                .id_salt(("service_report", report))
                .auto_shrink([false, true])
                .max_height(MAX_REPORT_HEIGHT)
                .show(ui, |ui| {
                    let response = ui.add(
                        Label::new(egui::RichText::new(report).color(palette(ui.ctx()).accent))
                            .selectable(true)
                            .sense(Sense::focusable_noninteractive()),
                    );
                    if response.enabled() && (response.clicked() || response.drag_started()) {
                        response.request_focus();
                    }
                    response
                });
            if body.inner.has_focus() {
                ui.painter().rect_stroke(
                    body.inner_rect,
                    0.0,
                    Stroke::new(1.0, palette(ui.ctx()).accent),
                    StrokeKind::Inside,
                );
            }
        });
    });
}

#[cfg(test)]
mod tests;
