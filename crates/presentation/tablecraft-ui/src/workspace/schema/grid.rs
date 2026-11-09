//! Fixed-height declaration cells retain their full read-only accessible text.
use crate::theme::palette;
use egui::{Id, Response, Sense, TextStyle, Ui, WidgetInfo, WidgetType};

const TEXT_PADDING: f32 = 3.0;
const GRID_LINE_WIDTH: f32 = 1.0;

#[derive(Clone, Copy)]
pub(super) enum CellStyle {
    Header { active: bool },
    FieldName { selected: bool, id: Id },
    Value,
    Unknown,
}

pub(super) fn cell(ui: &mut Ui, text: &str, style: CellStyle) -> Response {
    let colors = palette(ui.ctx());
    let sense = match style {
        CellStyle::FieldName { .. } => Sense::click(),
        CellStyle::Header { .. } | CellStyle::Value | CellStyle::Unknown => Sense::hover(),
    };
    let (rect, allocated) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    let response = match style {
        CellStyle::FieldName { id, .. } => ui.interact(rect, id, sense),
        CellStyle::Header { .. } | CellStyle::Value | CellStyle::Unknown => allocated,
    };
    let fill = match style {
        CellStyle::Header { active: true } => colors.active_header,
        CellStyle::Header { active: false } => colors.grid_header,
        CellStyle::FieldName { selected: true, .. } => colors.active_cell,
        CellStyle::FieldName {
            selected: false, ..
        }
        | CellStyle::Value
        | CellStyle::Unknown => colors.surface,
    };
    paint_grid(ui, rect, fill);

    let color = if matches!(style, CellStyle::Unknown) {
        colors.muted
    } else {
        colors.ink
    };
    // The layout API consumes this FontId; keep the original style or loop-local descriptor
    // available for the next measurement.
    let font = ui.style().text_styles[&TextStyle::Body].clone();
    let mut layout = egui::text::LayoutJob::simple(
        text.to_owned(),
        font,
        color,
        (rect.width() - TEXT_PADDING * 2.0).max(0.0),
    );
    layout.wrap.max_rows = 1;
    layout.wrap.break_anywhere = true;
    let galley = ui.painter().layout_job(layout);
    ui.painter().with_clip_rect(rect).galley(
        egui::pos2(
            if matches!(style, CellStyle::Header { .. }) {
                rect.center().x - galley.size().x * 0.5
            } else {
                rect.left() + TEXT_PADDING
            },
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        color,
    );
    match style {
        CellStyle::FieldName { selected, .. } => {
            response.widget_info(|| {
                WidgetInfo::selected(WidgetType::SelectableLabel, ui.is_enabled(), selected, text)
            });
            let enabled = ui.is_enabled();
            ui.ctx().accesskit_node_builder(response.id, |node| {
                node.clear_toggled();
                node.set_selected(selected);
                if !enabled {
                    node.remove_action(egui::accesskit::Action::Click);
                    node.remove_action(egui::accesskit::Action::Focus);
                }
            });
            if selected {
                ui.painter().rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(1.0, colors.accent),
                    egui::StrokeKind::Inside,
                );
            }
            if response.clicked() {
                response.request_focus();
            }
            if ui.is_enabled() && response.has_focus() {
                ui.memory_mut(|memory| {
                    memory.set_focus_lock_filter(
                        response.id,
                        egui::EventFilter {
                            vertical_arrows: true,
                            ..Default::default()
                        },
                    );
                });
            }
        }
        CellStyle::Header { .. } | CellStyle::Value | CellStyle::Unknown => {
            response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, ui.is_enabled(), text));
        }
    }
    response.on_hover_text(text)
}

/// Inert rows extend the visible grid without inventing native declarations.
pub(super) fn blank(ui: &mut Ui) {
    let rect = ui.max_rect();
    paint_grid(ui, rect, palette(ui.ctx()).surface);
    ui.allocate_rect(rect, Sense::hover());
}

pub(super) fn selector(ui: &mut Ui, selected: bool) {
    let rect = ui.max_rect();
    let colors = palette(ui.ctx());
    paint_grid(ui, rect, colors.grid_header);
    ui.allocate_rect(rect, Sense::hover());
    if selected {
        let center = rect.center();
        ui.painter().add(egui::Shape::convex_polygon(
            vec![
                center + egui::vec2(-2.0, -4.0),
                center + egui::vec2(3.0, 0.0),
                center + egui::vec2(-2.0, 4.0),
            ],
            colors.ink,
            egui::Stroke::NONE,
        ));
    }
}

fn paint_grid(ui: &Ui, rect: egui::Rect, fill: egui::Color32) {
    ui.painter().rect_filled(rect, 0.0, fill);
    let line = egui::Stroke::new(GRID_LINE_WIDTH, palette(ui.ctx()).grid_line);
    // Table cells clip at their edges. Center the complete stroke inside the
    // cell rather than losing the lower half beyond its clipping boundary.
    let ruled = rect.shrink(GRID_LINE_WIDTH * 0.5);
    ui.painter()
        .line_segment([ruled.right_top(), ruled.right_bottom()], line);
    ui.painter()
        .line_segment([ruled.left_bottom(), ruled.right_bottom()], line);
}
