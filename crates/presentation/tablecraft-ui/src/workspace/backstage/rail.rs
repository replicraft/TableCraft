//! Working File sections use a full-width rail with licensed command artwork.
use super::super::ribbon::Icon;
use crate::{Localizer, theme::palette};
use egui::{Response, Sense, TextStyle, Ui, WidgetInfo, WidgetType};

pub(super) const ROW_HEIGHT: f32 = 34.0;
pub(super) const OUTER_PADDING: i8 = 10;
const MIN_WIDTH: f32 = 160.0;
const MAX_WIDTH: f32 = 240.0;
const ICON_SIZE: f32 = 18.0;
const CONTENT_PADDING: f32 = 10.0;
const ICON_TEXT_GAP: f32 = 12.0;

/// Reserve translated interface labels without changing their wording or keys.
pub(super) fn width(ui: &Ui, localizer: &Localizer) -> f32 {
    // The layout API consumes this FontId; keep the original style or loop-local descriptor
    // available for the next measurement.
    let font = ui.style().text_styles[&TextStyle::Body].clone();
    let labels = [
        "backstage-back",
        "backstage-home",
        "backstage-open",
        "backstage-about",
        "backstage-options",
        "backstage-new",
        "backstage-info",
        "backstage-save",
        "backstage-save-as",
        "backstage-print",
        "backstage-close",
        "backstage-account",
    ];
    let text_width = labels
        .into_iter()
        .map(|key| {
            ui.painter()
                // The layout API consumes this FontId; keep the original style or loop-local
                // descriptor available for the next measurement.
                .layout_no_wrap(localizer.text(key), font.clone(), palette(ui.ctx()).ink)
                .size()
                .x
        })
        .fold(0.0, f32::max);
    let padding = 2.0 * f32::from(OUTER_PADDING) + 2.0 * CONTENT_PADDING;
    // Panel reserves its separator in the frame's outer margin. Scrolled rails
    // can also spend horizontal space on a non-floating scrollbar.
    let separator = ui.visuals().widgets.noninteractive.bg_stroke.width.round();
    let scroll_gutter = ui.spacing().scroll.allocated_width();
    (text_width.ceil() + ICON_SIZE + ICON_TEXT_GAP + padding + separator + scroll_gutter)
        .clamp(MIN_WIDTH, MAX_WIDTH)
}

pub(super) fn entry(ui: &mut Ui, icon: Icon, label: &str, selected: Option<bool>) -> Response {
    let colors = palette(ui.ctx());
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), ROW_HEIGHT), Sense::click());
    let active = selected == Some(true);
    let color = if !ui.is_enabled() {
        colors.muted
    } else if active {
        colors.accent
    } else {
        colors.ink
    };
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
    icon.image(ui, ICON_SIZE).paint_at(ui, icon_rect);
    let text_left = icon_rect.right() + ICON_TEXT_GAP;
    let mut layout = egui::text::LayoutJob::simple(
        label.to_owned(),
        // The layout API consumes this FontId; keep the original style or loop-local descriptor
        // available for the next measurement.
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
