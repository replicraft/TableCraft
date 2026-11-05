//! Real viewport requests for the custom Windows/Linux title strip.
use crate::Localizer;
use crate::theme::palette;
use egui::{Response, Sense, Stroke, StrokeKind, Ui, ViewportCommand, WidgetInfo, WidgetType};

const BUTTON_WIDTH: f32 = 36.0;
const BUTTON_HEIGHT: f32 = 30.0;
const GLYPH_HALF_SIZE: f32 = 5.0;
pub(super) const CONTROLS_WIDTH: f32 = BUTTON_WIDTH * 3.0;

#[derive(Clone, Copy)]
enum Control {
    Minimize,
    Maximize,
    Close,
}

pub(super) const fn enabled() -> bool {
    cfg!(any(target_os = "windows", target_os = "linux"))
}

pub(super) fn show(ui: &mut Ui, localizer: &Localizer) {
    let maximized = ui.input(|input| input.viewport().maximized.unwrap_or(false));
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for control in [Control::Minimize, Control::Maximize, Control::Close] {
            let key = match control {
                Control::Minimize => "shell-window-minimize",
                Control::Maximize if maximized => "shell-window-restore",
                Control::Maximize => "shell-window-maximize",
                Control::Close => "shell-window-close",
            };
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(BUTTON_WIDTH, BUTTON_HEIGHT), Sense::click());
            let colors = palette(ui.ctx());
            let close_hover = matches!(control, Control::Close) && response.hovered();
            if response.hovered() || response.has_focus() {
                let fill = if close_hover {
                    colors.accent
                } else {
                    colors.grid_header
                };
                ui.painter().rect_filled(rect, 0.0, fill);
            }
            let stroke = Stroke::new(
                1.0,
                if close_hover {
                    colors.surface
                } else {
                    colors.ink
                },
            );
            let glyph = egui::Rect::from_center_size(
                rect.center(),
                egui::Vec2::splat(GLYPH_HALF_SIZE * 2.0),
            );
            match control {
                Control::Minimize => {
                    ui.painter()
                        .line_segment([glyph.left_center(), glyph.right_center()], stroke);
                }
                Control::Maximize => {
                    if maximized {
                        let front = glyph.translate(egui::vec2(-1.0, 1.0)).shrink(1.0);
                        let back = front.translate(egui::vec2(2.0, -2.0));
                        ui.painter()
                            .rect_stroke(back, 0.0, stroke, StrokeKind::Inside);
                        ui.painter().rect_filled(front, 0.0, colors.title_fill);
                        ui.painter()
                            .rect_stroke(front, 0.0, stroke, StrokeKind::Inside);
                    } else {
                        ui.painter()
                            .rect_stroke(glyph, 0.0, stroke, StrokeKind::Inside);
                    }
                }
                Control::Close => {
                    ui.painter()
                        .line_segment([glyph.left_top(), glyph.right_bottom()], stroke);
                    ui.painter()
                        .line_segment([glyph.right_top(), glyph.left_bottom()], stroke);
                }
            }
            if response.has_focus() {
                ui.painter()
                    .rect_stroke(rect.shrink(1.0), 0.0, stroke, StrokeKind::Inside);
            }
            let label = localizer.text(key);
            response
                .widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), &label));
            if response.clicked() {
                let command = match control {
                    Control::Minimize => ViewportCommand::Minimized(true),
                    Control::Maximize => ViewportCommand::Maximized(!maximized),
                    Control::Close => ViewportCommand::Close,
                };
                ui.ctx().send_viewport_cmd(command);
            }
            response.on_hover_text(label);
        }
    });
}

pub(super) fn drag_title(response: &Response) {
    if !enabled() || !response.enabled() {
        return;
    }
    if response.drag_started() {
        response.ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }
    if response.double_clicked() {
        let maximized = response
            .ctx
            .input(|input| input.viewport().maximized.unwrap_or(false));
        response
            .ctx
            .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    }
}
