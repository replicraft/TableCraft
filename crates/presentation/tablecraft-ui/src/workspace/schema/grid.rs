//! Fixed-height declaration cells retain their full read-only accessible text.
use crate::theme::palette;
use egui::{Id, Response, Sense, TextStyle, Ui, WidgetInfo, WidgetType};

const TEXT_PADDING: f32 = 3.0;

#[derive(Clone, Copy)]
pub(super) enum CellStyle {
    Header { active: bool },
    FieldName { selected: bool, id: Id },
    Value,
}

pub(super) fn cell(ui: &mut Ui, text: &str, style: CellStyle) -> Response {
    let colors = palette(ui.ctx());
    let sense = match style {
        CellStyle::FieldName { .. } => Sense::click(),
        CellStyle::Header { .. } | CellStyle::Value => Sense::hover(),
    };
    let (rect, allocated) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    let response = match style {
        CellStyle::FieldName { id, .. } => ui.interact(rect, id, sense),
        CellStyle::Header { .. } | CellStyle::Value => allocated,
    };
    let fill = match style {
        CellStyle::Header { active: true } => colors.active_header,
        CellStyle::Header { active: false } => colors.grid_header,
        CellStyle::FieldName { selected: true, .. } => colors.active_cell,
        CellStyle::FieldName {
            selected: false, ..
        }
        | CellStyle::Value => colors.surface,
    };
    ui.painter().rect_filled(rect, 0.0, fill);
    let line = egui::Stroke::new(0.5, colors.grid_line);
    ui.painter()
        .line_segment([rect.right_top(), rect.right_bottom()], line);
    ui.painter()
        .line_segment([rect.left_bottom(), rect.right_bottom()], line);

    let font = ui.style().text_styles[&TextStyle::Body].clone();
    let mut layout = egui::text::LayoutJob::simple(
        text.to_owned(),
        font,
        colors.ink,
        (rect.width() - TEXT_PADDING * 2.0).max(0.0),
    );
    layout.wrap.max_rows = 1;
    layout.wrap.break_anywhere = true;
    let galley = ui.painter().layout_job(layout);
    ui.painter().with_clip_rect(rect).galley(
        egui::pos2(
            rect.left() + TEXT_PADDING,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        colors.ink,
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
        CellStyle::Header { .. } | CellStyle::Value => {
            response.widget_info(|| WidgetInfo::labeled(WidgetType::Label, ui.is_enabled(), text));
        }
    }
    response.on_hover_text(text)
}
