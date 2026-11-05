//! Session-only Navigation Pane group presentation.
use super::super::focus::{self, Pane};
use crate::localization::tr;
use egui::{Align2, Id, Sense, Stroke, TextStyle, Ui, WidgetInfo, WidgetType};

const HEADER_HEIGHT: f32 = 24.0;
const CHEVRON_PADDING: f32 = 9.0;
const CHEVRON_HALF_WIDTH: f32 = 3.0;

pub(super) fn tables(ui: &mut Ui) -> bool {
    let id = Id::new("navigation_tables_expanded");
    let mut expanded = ui
        .ctx()
        .data(|data| data.get_temp::<bool>(id))
        .unwrap_or(true);
    let label = tr(ui, "navigation-tables");
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), HEADER_HEIGHT),
        Sense::click(),
    );
    if response.clicked() {
        expanded = !expanded;
        ui.ctx().data_mut(|data| data.insert_temp(id, expanded));
    }
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), &label));
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_expanded(expanded);
    });
    let palette = crate::theme::palette(ui.ctx());
    if response.hovered() || response.has_focus() {
        ui.painter().rect_filled(rect, 0.0, palette.active_header);
    }
    ui.painter().text(
        rect.left_center() + egui::vec2(2.0, 0.0),
        Align2::LEFT_CENTER,
        &label,
        TextStyle::Button.resolve(ui.style()),
        palette.ink,
    );
    let center = egui::pos2(rect.right() - CHEVRON_PADDING, rect.center().y);
    let direction = if expanded { -1.0 } else { 1.0 };
    ui.painter().line(
        vec![
            center + egui::vec2(-CHEVRON_HALF_WIDTH, -direction),
            center + egui::vec2(0.0, direction * 2.0),
            center + egui::vec2(CHEVRON_HALF_WIDTH, -direction),
        ],
        Stroke::new(1.0, palette.muted),
    );
    focus::register(ui, Pane::Navigation, &response, false);
    expanded
}
