//! Inert field-property observations, distinct from native storage diagnostics.
use super::{Field, property, type_labels, yes_no};
use crate::workspace::focus::{self, Pane};
use crate::{Localizer, theme::palette};
use egui::text_selection::LabelSelectionState;
use egui::{Align, Direction, Event, Label, Layout, Response, ScrollArea, Ui, UiBuilder};
use egui_extras::{Column, TableBuilder};
use std::borrow::Cow;

const MAX_PROPERTY_COPY_BYTES: usize = 1_048_576;
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

#[derive(Clone, Default)]
struct PropertySelection {
    lookup: bool,
    selected: Option<String>,
    copy_error: Option<PropertyCopyFailure>,
}

pub(super) fn show(ui: &mut Ui, field: &Field) {
    let localizer = crate::localization::for_context(ui.ctx());
    let selection_id = ui.make_persistent_id(("property_selection", field.number, &field.name));
    let mut selection = ui.data_mut(|data| {
        data.get_temp::<PropertySelection>(selection_id)
            .unwrap_or_default()
    });

    ui.horizontal(|ui| {
        for (lookup, key) in [(false, "design-general"), (true, "design-lookup")] {
            let response = ui.selectable_label(selection.lookup == lookup, localizer.text(key));
            focus::register(ui, Pane::Properties, &response, false);
            if response.clicked() {
                selection.lookup = lookup;
                selection.selected = None;
                selection.copy_error = None;
            }
        }
    });

    let available = ui.available_size();
    let help_width = (available.x * 0.25)
        .clamp(120.0, 260.0)
        .min(available.x * 0.45);
    let sheet_width = (available.x - help_width - ui.spacing().item_spacing.x).max(1.0);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(sheet_width, available.y),
            Layout::top_down(Align::Min),
            |ui| {
                ScrollArea::both()
                    .id_salt("field_property_sheet")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if selection.lookup {
                            ui.weak(localizer.text("design-lookup-uninspected"));
                        } else {
                            show_general(ui, field, &mut selection);
                        }
                    });
            },
        );
        ui.allocate_ui_with_layout(
            egui::vec2(help_width, available.y),
            Layout::top_down(Align::Min),
            |ui| {
                ScrollArea::vertical()
                    .id_salt("field_property_help")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.weak(localizer.text(help_key(&selection)));
                    });
            },
        );
    });
    ui.data_mut(|data| data.insert_temp(selection_id, selection));
}

fn help_key(selection: &PropertySelection) -> &'static str {
    if selection.lookup {
        return "design-lookup-uninspected";
    }
    match selection.selected.as_deref() {
        Some("Required") => "design-help-required",
        Some("AllowZeroLength") => "design-help-zero-length",
        Some("DefaultValue") => "design-help-default",
        Some("ValidationRule" | "ValidationText") => "design-help-validation",
        Some("Caption" | "Description") => "design-help-label",
        Some("FieldSize") => "design-help-size",
        _ => "design-help-selection",
    }
}

fn show_general(ui: &mut Ui, field: &Field, selection: &mut PropertySelection) {
    let localizer = crate::localization::for_context(ui.ctx());
    let mut observations = Vec::with_capacity(field.properties.len() + 1);
    if let Some(size) = &field.field_size {
        observations.push((
            "FieldSize",
            localizer.text("design-field-size"),
            Cow::Owned(type_labels::field_size(&localizer, size)),
        ));
    }
    for observation in &field.properties {
        let value = match &observation.value {
            FieldPropertyValue::Uninspected => Cow::Owned(localizer.text("design-not-inspected")),
            FieldPropertyValue::Absent => Cow::Owned(localizer.text("design-not-stored")),
            FieldPropertyValue::Boolean(value) => Cow::Owned(yes_no(ui, *value)),
            FieldPropertyValue::Text(value) => Cow::Borrowed(value.as_str()),
            FieldPropertyValue::Unsupported => {
                Cow::Owned(localizer.text("design-unsupported-value"))
            }
        };
        observations.push((observation.name, observation.label(&localizer), value));
    }
    if observations.is_empty() {
        ui.weak(localizer.text("design-no-observations"));
    } else {
        if let Some(failure) = general_sheet(ui, &observations, selection) {
            selection.copy_error = Some(failure);
        }
        if let Some(failure) = selection.copy_error {
            ui.label(localizer.text(failure.key()));
        }
    }

    ui.add_space(ui.spacing().item_spacing.y);
    let details =
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
                    let scalar_type = field.scalar_type.as_deref().map_or_else(
                        || Cow::Owned(localizer.text("design-unsupported")),
                        Cow::Borrowed,
                    );
                    property(
                        ui,
                        &localizer.text("design-readable-carrier"),
                        scalar_type.as_ref(),
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
    focus::register(ui, Pane::Properties, &details.header_response, false);
    if details.header_response.gained_focus() {
        details.header_response.scroll_to_me(None);
    }
}

fn general_sheet(
    ui: &mut Ui,
    observations: &[(&str, String, Cow<'_, str>)],
    selection: &mut PropertySelection,
) -> Option<PropertyCopyFailure> {
    let mut failure = None;
    // NO_CLONE: The layout API consumes this FontId; keep the original style or loop-local
    // descriptor available for the next measurement.
    let font = ui.style().text_styles[&egui::TextStyle::Body].clone();
    let label_width = observations
        .iter()
        .fold(PROPERTY_LABEL_MIN_WIDTH, |width, (_, label, _)| {
            // NO_CLONE: Two real toolkit outputs retain the text independently (paint/layout,
            // accessibility or hover/clipboard); the owning API consumes the String
            // while the other output remains needed. The layout API consumes this
            // FontId; keep the original style or loop-local descriptor available for
            // the next measurement.
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
                    let first = row.index() == 0;
                    let (key, label, value) = &observations[row.index()];
                    let mut label_id = None;
                    let mut focus_value = false;
                    row.col(|ui| {
                        let response = property_cell(ui, label, false);
                        label_id = Some(response.id);
                        focus_value = response.clicked();
                        if response.clicked() || response.gained_focus() {
                            if selection.selected.as_deref() != Some(*key) {
                                selection.copy_error = None;
                            }
                            selection.selected = Some((*key).to_owned());
                        }
                    });
                    row.col(|ui| {
                        let response = property_cell(ui, value, true);
                        if focus_value {
                            response.request_focus();
                        }
                        if let Some(error) = copy_focused_value(ui, &response, value) {
                            failure = Some(error);
                        }
                        focus::register(
                            ui,
                            Pane::Properties,
                            &response,
                            response.has_focus() || first,
                        );
                        if response.clicked() || response.gained_focus() {
                            if selection.selected.as_deref() != Some(*key) {
                                selection.copy_error = None;
                            }
                            selection.selected = Some((*key).to_owned());
                        }
                        if let Some(label_id) = label_id {
                            response.labelled_by(label_id);
                        }
                    });
                });
            });
    });
    failure
}

#[derive(Clone, Copy)]
enum PropertyCopyFailure {
    Limit,
    Allocation,
}

impl PropertyCopyFailure {
    fn key(self) -> &'static str {
        match self {
            Self::Limit => "design-copy-limit",
            Self::Allocation => "design-copy-allocation",
        }
    }
}

fn copy_focused_value(
    ui: &mut Ui,
    response: &Response,
    value: &str,
) -> Option<PropertyCopyFailure> {
    // An explicit mouse-selected subset keeps egui's ordinary label-copy path.
    // Keyboard focus without a subset copies this observed property, never a
    // previously selected datasheet cell or another field's cached value.
    if !response.has_focus()
        || ui
            .ctx()
            .plugin::<LabelSelectionState>()
            .lock()
            .has_selection()
    {
        return None;
    }
    let copy = ui.input_mut(|input| {
        let requested = input
            .events
            .iter()
            .any(|event| matches!(event, Event::Copy));
        if requested {
            input.events.retain(|event| !matches!(event, Event::Copy));
        }
        requested
    });
    if !copy {
        return None;
    }
    if value.len() > MAX_PROPERTY_COPY_BYTES {
        return Some(PropertyCopyFailure::Limit);
    }
    let mut text = String::new();
    if text.try_reserve_exact(value.len()).is_err() {
        return Some(PropertyCopyFailure::Allocation);
    }
    text.push_str(value);
    ui.ctx().copy_text(text);
    None
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
    let response = ui
        .scope_builder(
            UiBuilder::new().max_rect(content).layout(
                Layout::centered_and_justified(Direction::TopDown).with_cross_align(Align::LEFT),
            ),
            |ui| {
                ui.add(
                    Label::new(text)
                        .selectable(selectable)
                        .sense(if selectable {
                            egui::Sense::click()
                        } else {
                            egui::Sense::CLICK
                        })
                        .halign(Align::LEFT)
                        .truncate(),
                )
            },
        )
        .inner
        .on_hover_text(text);
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(1.0, colors.accent),
            egui::StrokeKind::Inside,
        );
    }
    response
}

#[cfg(test)]
mod tests;
