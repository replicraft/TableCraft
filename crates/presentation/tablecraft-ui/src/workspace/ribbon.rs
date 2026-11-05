//! Dense Access-style command groups; mock commands remain disabled and inert.
mod icons;
mod mock_groups;

pub(super) use icons::Icon;
pub(super) use mock_groups::{design, home, mock as mock_command};

use crate::theme::palette;
use egui::{Align, FontId, Response, RichText, Sense, Ui, WidgetInfo, WidgetType};

const COMMAND_AREA_HEIGHT: f32 = 64.0;
const LARGE_COMMAND_WIDTH: f32 = 66.0;
const SMALL_COMMAND_HEIGHT: f32 = 20.0;
const LARGE_ICON_SIZE: f32 = 28.0;
const SMALL_ICON_SIZE: f32 = 16.0;
const LABEL_SIZE: f32 = 12.0;
const GROUP_LABEL_HEIGHT: f32 = 14.0;
const SCROLLBAR_THICKNESS: f32 = 6.0;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Tab {
    #[default]
    Home,
    TableDesign,
}

#[derive(Clone, Copy)]
pub(super) enum CommandSize {
    Small,
    Large,
    IconOnly,
}

pub(super) fn strip(ui: &mut Ui, contents: impl FnOnce(&mut Ui)) {
    ui.scope(|ui| {
        ui.spacing_mut().scroll.floating = false;
        ui.spacing_mut().scroll.bar_width = SCROLLBAR_THICKNESS;
        egui::ScrollArea::horizontal()
            .id_salt("ribbon_commands")
            .auto_shrink([false, true])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    contents(ui);
                });
            });
    });
}

pub(super) fn group(ui: &mut Ui, label: &str, contents: impl FnOnce(&mut Ui)) {
    let colors = palette(ui.ctx());
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        ui.set_min_height(COMMAND_AREA_HEIGHT);
        let start = ui.cursor().top();
        contents(ui);
        let remaining = COMMAND_AREA_HEIGHT - (ui.cursor().top() - start);
        if remaining > 0.0 {
            ui.add_space(remaining);
        }
        let width = ui.min_rect().width();
        ui.allocate_ui_with_layout(
            egui::vec2(width, GROUP_LABEL_HEIGHT),
            egui::Layout::top_down(Align::Center),
            |ui| {
                ui.add(
                    egui::Label::new(RichText::new(label).size(10.5).color(colors.muted))
                        .wrap_mode(egui::TextWrapMode::Extend),
                );
            },
        );
    });
    ui.separator();
}

pub(super) fn command(
    ui: &mut Ui,
    icon: Icon,
    label: &str,
    enabled: bool,
    size: CommandSize,
) -> Response {
    ui.add_enabled_ui(enabled, |ui| {
        let colors = palette(ui.ctx());
        let color = if ui.is_enabled() {
            colors.ink
        } else {
            colors.muted
        };
        let font = FontId::proportional(LABEL_SIZE);
        let (desired, text_width) = match size {
            CommandSize::Large => {
                let width = large_command_width(ui, label, &font, color);
                (egui::vec2(width, COMMAND_AREA_HEIGHT), width - 6.0)
            }
            CommandSize::Small => {
                let text = ui
                    .painter()
                    .layout_no_wrap(label.to_owned(), font.clone(), color);
                (
                    egui::vec2(SMALL_ICON_SIZE + text.size().x + 10.0, SMALL_COMMAND_HEIGHT),
                    f32::INFINITY,
                )
            }
            CommandSize::IconOnly => (egui::vec2(24.0, SMALL_COMMAND_HEIGHT), 0.0),
        };
        let (rect, response) = ui.allocate_exact_size(desired, Sense::click());
        if ui.is_enabled() && (response.hovered() || response.has_focus()) {
            let visuals = ui.style().interact(&response);
            ui.painter().rect_filled(rect, 2.0, visuals.weak_bg_fill);
            if response.has_focus() {
                ui.painter().rect_stroke(
                    rect,
                    2.0,
                    egui::Stroke::new(1.0, colors.accent),
                    egui::StrokeKind::Inside,
                );
            }
        }
        let mut layout = egui::text::LayoutJob::simple(label.to_owned(), font, color, text_width);
        layout.wrap.max_rows = match size {
            CommandSize::Large => 2,
            CommandSize::Small | CommandSize::IconOnly => 1,
        };
        let text = ui.painter().layout_job(layout);
        let (image_rect, text_position) = match size {
            CommandSize::Large => (
                egui::Rect::from_center_size(
                    egui::pos2(rect.center().x, rect.top() + LARGE_ICON_SIZE * 0.5 + 3.0),
                    egui::vec2(LARGE_ICON_SIZE, LARGE_ICON_SIZE),
                ),
                egui::pos2(rect.center().x - text.size().x * 0.5, rect.top() + 34.0),
            ),
            CommandSize::Small => (
                egui::Rect::from_center_size(
                    egui::pos2(rect.left() + SMALL_ICON_SIZE * 0.5 + 2.0, rect.center().y),
                    egui::vec2(SMALL_ICON_SIZE, SMALL_ICON_SIZE),
                ),
                egui::pos2(
                    rect.left() + SMALL_ICON_SIZE + 6.0,
                    rect.center().y - text.size().y * 0.5,
                ),
            ),
            CommandSize::IconOnly => (
                egui::Rect::from_center_size(
                    rect.center(),
                    egui::vec2(SMALL_ICON_SIZE, SMALL_ICON_SIZE),
                ),
                rect.center(),
            ),
        };
        icon.image(color, image_rect.width())
            .paint_at(ui, image_rect);
        if !matches!(size, CommandSize::IconOnly) {
            ui.painter().galley(text_position, text, color);
        }
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), label));
        response
    })
    .inner
}

fn large_command_width(ui: &Ui, label: &str, font: &FontId, color: egui::Color32) -> f32 {
    let measure = |text: &str| {
        ui.painter()
            .layout_no_wrap(text.to_owned(), font.clone(), color)
            .size()
            .x
    };
    let mut text_width = measure(label);

    // Choose a real word boundary for two complete lines. A fixed English-sized
    // button can otherwise elide translated commands even when two lines fit.
    for (offset, character) in label.char_indices() {
        if character.is_whitespace() {
            let first = label[..offset].trim_end();
            let second = label[offset..].trim_start();
            if !first.is_empty() && !second.is_empty() {
                text_width = text_width.min(measure(first).max(measure(second)));
            }
        }
    }

    LARGE_COMMAND_WIDTH.max(text_width.ceil() + 8.0)
}
