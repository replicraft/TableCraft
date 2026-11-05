//! Inert field-property observations, distinct from native storage diagnostics.
use super::{Field, property, yes_no};
use crate::{Localizer, theme::palette};
use egui::{Align, Direction, Frame, Label, Layout, Margin, Response, RichText, Ui, UiBuilder};
use egui_extras::{Column, TableBuilder};

const PROPERTY_ROW_HEIGHT: f32 = 18.0;
const PROPERTY_LABEL_MIN_WIDTH: f32 = 130.0;
const PROPERTY_LABEL_MAX_WIDTH: f32 = 260.0;
const PROPERTY_TEXT_PADDING: f32 = 3.0;

/// A named observation from the current metadata inspection.
pub struct FieldProperty {
    pub name: &'static str,
    pub value: FieldPropertyValue,
}

impl FieldProperty {
    /// Presentation labels do not replace the canonical native property key.
    pub fn label(&self, localizer: &Localizer) -> String {
        let key = match self.name {
            "Required" => "design-required",
            "AllowZeroLength" => "design-allow-zero-length",
            "DefaultValue" => "design-default-value",
            "ValidationRule" => "design-validation-rule",
            "ValidationText" => "design-validation-text",
            "Caption" => "design-caption",
            "Description" => "design-description",
            name => return name.to_owned(),
        };
        localizer.text(key)
    }
}

/// Missing, uninspected and unsupported metadata must remain distinguishable.
pub enum FieldPropertyValue {
    Uninspected,
    Absent,
    Boolean(bool),
    Text(String),
    Unsupported,
}

pub(super) fn show(ui: &mut Ui, field: &Field) {
    let localizer = crate::localization::for_context(ui.ctx());
    let colors = palette(ui.ctx());
    Frame::new()
        .fill(colors.surface)
        .stroke(egui::Stroke::new(0.5, colors.grid_line))
        .inner_margin(Margin::symmetric(4, 1))
        .show(ui, |ui| {
            ui.label(RichText::new(localizer.text("design-general")));
        });

    let mut observations = Vec::with_capacity(field.properties.len() + 1);
    if let Some(size) = &field.field_size {
        observations.push((localizer.text("design-field-size"), size.clone()));
    }
    for observation in &field.properties {
        let value = match &observation.value {
            FieldPropertyValue::Uninspected => localizer.text("design-not-inspected"),
            FieldPropertyValue::Absent => localizer.text("design-not-stored"),
            FieldPropertyValue::Boolean(value) => yes_no(ui, *value),
            FieldPropertyValue::Text(value) => value.clone(),
            FieldPropertyValue::Unsupported => localizer.text("design-unsupported-value"),
        };
        observations.push((observation.label(&localizer), value));
    }
    if observations.is_empty() {
        ui.weak(localizer.text("design-no-observations"));
    } else {
        general_sheet(ui, &observations);
    }

    ui.add_space(ui.spacing().item_spacing.y);
    egui::CollapsingHeader::new(localizer.text("design-native-details")).show(ui, |ui| {
        egui::Grid::new("native_field_properties")
            .num_columns(2)
            .show(ui, |ui| {
                property(
                    ui,
                    &localizer.text("design-property-field-name"),
                    &field.name,
                );
                property(
                    ui,
                    &localizer.text("design-field-number"),
                    field.number.to_string(),
                );
                property(
                    ui,
                    &localizer.text("design-native-type"),
                    format!("0x{:02X}", field.native_type),
                );
                property(
                    ui,
                    &localizer.text("design-readable-carrier"),
                    field
                        .scalar_type
                        .clone()
                        .unwrap_or_else(|| localizer.text("design-unsupported")),
                );
                property(
                    ui,
                    &localizer.text("design-storage-bytes"),
                    field.storage_bytes.to_string(),
                );
                let fixed_storage = yes_no(ui, field.fixed);
                property(ui, &localizer.text("design-fixed-storage"), fixed_storage);
                property(
                    ui,
                    &localizer.text("design-native-flags"),
                    format!("0x{:02X}", field.flags),
                );
                property(
                    ui,
                    &localizer.text("design-extended-flags"),
                    format!("0x{:02X}", field.extended_flags),
                );
                let nullable = field
                    .reader_nullable
                    .map(|value| yes_no(ui, value))
                    .unwrap_or_else(|| localizer.text("design-unclassified"));
                property(ui, &localizer.text("design-reader-null"), nullable);
                if let Some((precision, scale)) = field.decimal_parameters {
                    let parameters = crate::localization::message(
                        ui,
                        "design-precision-value",
                        &[
                            ("precision", precision.to_string()),
                            ("scale", scale.to_string()),
                        ],
                    );
                    property(ui, &localizer.text("design-precision-scale"), parameters);
                }
                if let Some(id) = field.complex_type_id {
                    property(ui, &localizer.text("design-complex-id"), id.to_string());
                }
            });
        ui.small(localizer.text("design-reader-null-help"));
    });
}

fn general_sheet(ui: &mut Ui, observations: &[(String, String)]) {
    let font = ui.style().text_styles[&egui::TextStyle::Body].clone();
    let label_width = observations
        .iter()
        .fold(PROPERTY_LABEL_MIN_WIDTH, |width, (label, _)| {
            let galley =
                ui.painter()
                    .layout_no_wrap(label.clone(), font.clone(), palette(ui.ctx()).ink);
            width.max(galley.size().x + PROPERTY_TEXT_PADDING * 2.0)
        })
        .min(PROPERTY_LABEL_MAX_WIDTH);

    ui.scope(|ui| {
        ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
        TableBuilder::new(ui)
            .id_salt("general_field_properties")
            .vscroll(false)
            .striped(false)
            .auto_shrink([false, true])
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::exact(label_width))
            .column(Column::remainder())
            .body(|body| {
                body.rows(PROPERTY_ROW_HEIGHT, observations.len(), |mut row| {
                    let (label, value) = &observations[row.index()];
                    let mut label_id = None;
                    row.col(|ui| {
                        label_id = Some(property_cell(ui, label, false).id);
                    });
                    row.col(|ui| {
                        let response = property_cell(ui, value, true);
                        if let Some(label_id) = label_id {
                            response.labelled_by(label_id);
                        }
                    });
                });
            });
    });
}

fn property_cell(ui: &mut Ui, text: &str, selectable: bool) -> Response {
    let colors = palette(ui.ctx());
    let rect = ui.max_rect();
    let stroke = egui::Stroke::new(0.5, colors.grid_line);
    ui.painter().rect_filled(rect, 0.0, colors.surface);
    ui.painter()
        .rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Inside);

    let content = rect.shrink2(egui::vec2(PROPERTY_TEXT_PADDING, 0.0));
    // Keep the full cell response/selection region, but align the label's
    // exact-size galley at the left. Ui::put centers that inner galley even
    // when Label::halign is LEFT.
    ui.scope_builder(
        UiBuilder::new().max_rect(content).layout(
            Layout::centered_and_justified(Direction::TopDown).with_cross_align(Align::LEFT),
        ),
        |ui| {
            ui.add(
                Label::new(text)
                    .selectable(selectable)
                    .halign(Align::LEFT)
                    .truncate(),
            )
        },
    )
    .inner
    .on_hover_text(text)
}
