//! The source area and QBE rows follow the retained native Design captures.
use super::grid::{GRID_ROW_HEIGHT, design_grid};
use super::sources::source_area;
use crate::localization::tr;
use crate::workspace::{State, diagnostics};
use egui::Ui;

const MIN_SOURCE_HEIGHT: f32 = 80.0;
const MIN_GRID_HEIGHT: f32 = 180.0;
const SPLITTER_HEIGHT: f32 = 7.0;

impl State {
    pub(in crate::workspace) fn query_design_editor(&mut self, ui: &mut Ui) {
        if !self.query_design_visible() {
            return;
        }
        let ready = self.query_design_ready();
        let mut session = self
            .query_design
            .session
            .take()
            .expect("visible Design session");
        ui.add_enabled_ui(ready, |ui| {
            let source_height = source_height(ui);
            let mut changed = source_area(ui, &mut session, source_height);
            source_splitter(ui, source_height);
            changed |= design_grid(ui, &mut session);
            if changed {
                session.revision = session.revision.wrapping_add(1);
            }
        });
        self.query_design.session = Some(session);
    }

    pub(in crate::workspace) fn query_design_report(&self, ui: &mut Ui) {
        if self.query_design_request().is_none() {
            return;
        }
        if let Some(error) = self
            .query_design
            .session
            .as_ref()
            .and_then(|session| session.error.as_ref())
        {
            diagnostics::show(ui, ui.make_persistent_id("query_design_error"), error);
        }
    }
}

fn source_height(ui: &Ui) -> f32 {
    let maximum =
        (ui.available_height() - MIN_GRID_HEIGHT - SPLITTER_HEIGHT).max(MIN_SOURCE_HEIGHT);
    ui.data(|data| data.get_temp::<f32>(ui.make_persistent_id("qbe_source_height")))
        .unwrap_or(ui.available_height() * 0.45)
        .clamp(MIN_SOURCE_HEIGHT, maximum)
}

fn source_splitter(ui: &mut Ui, height: f32) {
    let maximum =
        (height + ui.available_height() - MIN_GRID_HEIGHT - SPLITTER_HEIGHT).max(MIN_SOURCE_HEIGHT);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), SPLITTER_HEIGHT),
        egui::Sense::click_and_drag(),
    );
    let response = response.on_hover_cursor(egui::CursorIcon::ResizeVertical);
    let label = tr(ui, "qbe-resize-panes");
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &label)
    });
    if response.clicked() {
        response.request_focus();
    }
    let keyboard_delta = if response.has_focus() {
        ui.input_mut(|input| {
            let down = input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown);
            let up = input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp);
            (i32::from(down) - i32::from(up)) as f32 * GRID_ROW_HEIGHT
        })
    } else {
        0.0
    };
    if response.dragged() || keyboard_delta != 0.0 {
        let next =
            (height + response.drag_delta().y + keyboard_delta).clamp(MIN_SOURCE_HEIGHT, maximum);
        ui.data_mut(|data| data.insert_temp(ui.make_persistent_id("qbe_source_height"), next));
        ui.ctx().request_repaint();
    }
    let colors = crate::theme::palette(ui.ctx());
    let stroke = egui::Stroke::new(
        1.0,
        if response.hovered() || response.has_focus() {
            colors.accent
        } else {
            colors.grid_line
        },
    );
    ui.painter()
        .line_segment([rect.left_center(), rect.right_center()], stroke);
}

#[cfg(test)]
mod tests;
