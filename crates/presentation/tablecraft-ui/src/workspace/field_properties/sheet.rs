//! A ruled property sheet keeps its value cells aligned with the help-pane boundary.
use super::PROPERTY_INPUT_MARGIN;
use crate::{localization::tr, theme::palette};
use egui::{Align, Layout, Rect, Stroke, Ui};

const MAX_LABEL_WIDTH_FRACTION: f32 = 0.42;
const LABEL_PADDING: f32 = 8.0;
const RULE_WIDTH: f32 = 1.0;

const LABELS: [&str; 10] = [
    "design-field-size",
    "design-required",
    "design-allow-zero-length",
    "design-default-value",
    "table-draft-property-literal-input",
    "table-draft-number-precision",
    "table-draft-number-scale",
    "design-caption",
    "design-validation-rule",
    "design-validation-text",
];

pub(in crate::workspace) struct Sheet {
    left: f32,
    width: f32,
    label_width: f32,
    row_height: f32,
    top: Option<f32>,
    row_top: f32,
    row_bottoms: Vec<f32>,
}

impl Sheet {
    pub(in crate::workspace) fn new(ui: &Ui) -> Self {
        let font = egui::TextStyle::Body.resolve(ui.style());
        let width = ui.available_width();
        let label_width = LABELS
            .iter()
            .map(|key| {
                // NO_CLONE: The layout API consumes this FontId; keep the original style or
                // loop-local descriptor available for the next measurement.
                ui.painter()
                    .layout_no_wrap(tr(ui, key), font.clone(), ui.visuals().text_color())
                    .size()
                    .x
                    + LABEL_PADDING
            })
            .fold(0.0, f32::max)
            .min(width * MAX_LABEL_WIDTH_FRACTION);
        let input_padding =
            f32::from(PROPERTY_INPUT_MARGIN.top) + f32::from(PROPERTY_INPUT_MARGIN.bottom);
        let input_height = (ui.text_style_height(&egui::TextStyle::Body)
            + ui.spacing().extra_text_line_spacing
            + input_padding)
            .max(ui.spacing().interact_size.y);
        let pixels_per_point = ui.ctx().pixels_per_point();
        let row_height = (input_height * pixels_per_point).ceil() / pixels_per_point;
        Self {
            left: ui.next_widget_position().x,
            width,
            label_width,
            row_height,
            top: None,
            row_top: ui.next_widget_position().y,
            row_bottoms: Vec::new(),
        }
    }

    pub(in crate::workspace) fn value_width(&self) -> f32 {
        (self.width - self.label_width).max(1.0)
    }

    pub(in crate::workspace) fn row_height(&self) -> f32 {
        self.row_height
    }

    pub(in crate::workspace) fn label(&mut self, ui: &mut Ui, key: &str, enabled: bool) {
        let text = tr(ui, key);
        self.label_text(ui, text, enabled);
    }

    pub(in crate::workspace) fn empty_label(&mut self, ui: &mut Ui) {
        self.label_text(ui, String::new(), true);
    }

    fn label_text(&mut self, ui: &mut Ui, text: String, enabled: bool) {
        let response = ui.add_enabled(enabled, |ui: &mut Ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(self.label_width, self.row_height),
                Layout::left_to_right(Align::Center),
                |ui| {
                    // allocate_ui_with_layout permits shrinking to contents;
                    // keep the full measured cell, including its padding.
                    ui.set_min_size(egui::vec2(self.label_width, self.row_height));
                    ui.add(egui::Label::new(text).wrap());
                },
            )
            .response
        });
        self.row_top = response.rect.top();
        self.top.get_or_insert(self.row_top);
    }

    pub(in crate::workspace) fn end_row(&mut self, ui: &mut Ui) {
        let bounds = ui.min_rect();
        // Input frames snap to device pixels independently of the label UI.
        // Include the actual values before fixing the sheet's top boundary.
        self.top = Some(self.top.map_or(bounds.top(), |top| top.min(bounds.top())));
        self.row_bottoms.push(bounds.bottom().max(self.row_top));
        ui.end_row();
    }

    pub(in crate::workspace) fn paint(&self, ui: &Ui) {
        let Some(top) = self.top else {
            return;
        };
        let Some(bottom) = self.row_bottoms.last().copied() else {
            return;
        };
        let bounds = Rect::from_min_max(
            egui::pos2(self.left, top),
            egui::pos2(self.left + self.width, bottom),
        );
        let painter = ui
            .painter()
            .with_clip_rect(ui.clip_rect().intersect(bounds));
        let stroke = Stroke::new(RULE_WIDTH, palette(ui.ctx()).grid_line);
        for x in [
            bounds.left(),
            bounds.left() + self.label_width,
            bounds.right(),
        ] {
            painter.vline(x, bounds.y_range(), stroke);
        }
        painter.hline(bounds.x_range(), bounds.top(), stroke);
        for bottom in &self.row_bottoms {
            painter.hline(bounds.x_range(), *bottom, stroke);
        }
    }
}
