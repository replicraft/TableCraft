//! Object-row presentation never substitutes a caption for its source identity.
use super::super::ribbon::Icon;
use crate::theme::palette;
use egui::{Response, Sense, TextStyle, Ui, WidgetInfo, WidgetType};

const ROW_HEIGHT: f32 = 23.0;
const ICON_SIZE: f32 = 16.0;
const CONTENT_PADDING: f32 = 5.0;
const ICON_TEXT_GAP: f32 = 7.0;

pub(super) fn show(
    ui: &mut Ui,
    icon: Icon,
    name: &str,
    selected: bool,
    badge: Option<&str>,
) -> Response {
    let colors = palette(ui.ctx());
    let (_, rect) = ui.allocate_space(egui::vec2(ui.available_width(), ROW_HEIGHT));
    let response = ui.interact(rect, ui.make_persistent_id("object_row"), Sense::click());
    if selected {
        let fill = if response.has_focus() {
            colors.active_row
        } else {
            colors.surface
        };
        ui.painter().rect_filled(rect, 0.0, fill);
        ui.painter().rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.0, colors.grid_line),
            egui::StrokeKind::Inside,
        );
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, 0.0, ui.visuals().widgets.hovered.weak_bg_fill);
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
    icon.image(colors.ink, ICON_SIZE).paint_at(ui, icon_rect);
    let text_left = icon_rect.right() + ICON_TEXT_GAP;
    let description = badge.map(str::to_owned);
    let badge_color = if selected && response.has_focus() {
        colors.ink
    } else {
        colors.muted
    };
    let badge = badge.map(|label| {
        ui.painter().layout_no_wrap(
            label.to_owned(),
            ui.style().text_styles[&TextStyle::Small].clone(),
            badge_color,
        )
    });
    let text_right = badge
        .as_ref()
        .map_or(rect.right() - CONTENT_PADDING, |badge| {
            rect.right() - CONTENT_PADDING * 2.0 - badge.size().x
        });
    if let Some(badge) = &badge {
        ui.painter().with_clip_rect(rect).galley(
            egui::pos2(
                rect.right() - CONTENT_PADDING - badge.size().x,
                rect.center().y - badge.size().y * 0.5,
            ),
            badge.clone(),
            badge_color,
        );
    }
    let mut layout = egui::text::LayoutJob::simple(
        name.to_owned(),
        ui.style().text_styles[&TextStyle::Body].clone(),
        colors.ink,
        (text_right - text_left).max(0.0),
    );
    layout.wrap.max_rows = 1;
    layout.wrap.break_anywhere = true;
    let text = ui.painter().layout_job(layout);
    ui.painter().with_clip_rect(rect).galley(
        egui::pos2(text_left, rect.center().y - text.size().y * 0.5),
        text,
        colors.ink,
    );
    response
        .widget_info(|| WidgetInfo::selected(WidgetType::Button, ui.is_enabled(), selected, name));
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.clear_toggled();
        node.set_selected(selected);
        if let Some(description) = description {
            node.set_description(description);
        }
        if !ui.is_enabled() {
            node.remove_action(egui::accesskit::Action::Click);
            node.remove_action(egui::accesskit::Action::Focus);
        }
    });
    if response.clicked() {
        response.request_focus();
    }
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
    response.on_hover_text(name)
}

#[cfg(test)]
mod tests;
