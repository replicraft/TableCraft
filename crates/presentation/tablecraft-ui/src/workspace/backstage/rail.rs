//! Working File sections use a full-width rail with licensed command artwork.
use super::super::ribbon::Icon;
use crate::theme::palette;
use egui::{Response, Sense, TextStyle, Ui, WidgetInfo, WidgetType};

pub(super) const ROW_HEIGHT: f32 = 34.0;
const ICON_SIZE: f32 = 18.0;
const CONTENT_PADDING: f32 = 10.0;
const ICON_TEXT_GAP: f32 = 12.0;

pub(super) fn entry(ui: &mut Ui, icon: Icon, label: &str, selected: Option<bool>) -> Response {
    let colors = palette(ui.ctx());
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
    let active = selected == Some(true);
    let color = if active { colors.accent } else { colors.ink };
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 0.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    if active {
        ui.painter().line_segment(
            [rect.left_top(), rect.left_bottom()],
            egui::Stroke::new(2.0, colors.accent),
        );
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.0, colors.accent),
            egui::StrokeKind::Inside,
        );
    }

    let icon_rect = egui::Rect::from_center_size(
        egui::pos2(
            rect.left() + CONTENT_PADDING + ICON_SIZE * 0.5,
            rect.center().y,
        ),
        egui::vec2(ICON_SIZE, ICON_SIZE),
    );
    icon.image(color, ICON_SIZE).paint_at(ui, icon_rect);
    let text_left = icon_rect.right() + ICON_TEXT_GAP;
    let mut layout = egui::text::LayoutJob::simple(
        label.to_owned(),
        ui.style().text_styles[&TextStyle::Body].clone(),
        color,
        (rect.right() - CONTENT_PADDING - text_left).max(0.0),
    );
    layout.wrap.max_rows = 1;
    layout.wrap.break_anywhere = true;
    let text = ui.painter().layout_job(layout);
    ui.painter().with_clip_rect(rect).galley(
        egui::pos2(text_left, rect.center().y - text.size().y * 0.5),
        text,
        color,
    );
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), label));
    ui.ctx().accesskit_node_builder(response.id, |node| {
        if let Some(selected) = selected {
            node.set_role(egui::accesskit::Role::Tab);
            node.set_selected(selected);
        }
        if !ui.is_enabled() {
            node.remove_action(egui::accesskit::Action::Click);
            node.remove_action(egui::accesskit::Action::Focus);
        }
    });
    if response.clicked() {
        response.request_focus();
    }
    response.on_hover_text(label)
}
