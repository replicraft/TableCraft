//! Read-only Table Design uses host-supplied facts without interpreting storage.
mod grid;
mod properties;
pub use properties::{FieldProperty, FieldPropertyValue};

use super::State;
use egui::{RichText, ScrollArea, Ui};
use egui_extras::{Column, TableBuilder};

/// Captured declaration metadata, separate from table pages and query results.
pub struct Schema {
    pub table: String,
    pub declared_rows: u32,
    pub fields: Vec<Field>,
}

/// Native declaration facts and the backend's current reader classification.
pub struct Field {
    pub name: String,
    pub number: usize,
    pub native_type: u8,
    pub storage_bytes: usize,
    pub fixed: bool,
    pub flags: u8,
    pub extended_flags: u8,
    pub scalar_type: Option<String>,
    /// Optional host-supplied Access presentation name, never inferred from flags.
    pub display_type: Option<String>,
    pub field_size: Option<String>,
    pub reader_nullable: Option<bool>,
    pub decimal_parameters: Option<(u8, u8)>,
    pub complex_type_id: Option<u32>,
    pub properties: Vec<FieldProperty>,
}

impl State {
    pub(super) fn design(&mut self, ui: &mut Ui) {
        let localizer = self.localizer.clone();
        let Some(schema) = &self.schema else {
            ui.weak(crate::localization::tr(ui, "design-choose"));
            return;
        };
        ui.small(crate::localization::tr(ui, "design-read-only"))
            .on_hover_text(crate::localization::tr(ui, "design-inert-help"));
        if schema.fields.is_empty() {
            ui.weak(crate::localization::tr(ui, "design-no-fields"));
            return;
        }

        egui::Resize::default()
            .id_salt("schema_fields")
            .default_width(ui.available_width())
            .min_width(ui.available_width())
            .max_width(ui.available_width())
            .default_height((ui.available_height() * 0.68).max(120.0))
            .min_height(90.0)
            .max_height((ui.available_height() - 140.0).max(90.0))
            .resizable([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
                TableBuilder::new(ui)
                    .striped(false)
                    .resizable(true)
                    .column(Column::initial(210.0).at_least(100.0))
                    .column(Column::initial(140.0).at_least(90.0))
                    .column(Column::remainder().at_least(70.0))
                    .header(22.0, |mut header| {
                        for (position, title) in [
                            localizer.text("design-field-name"),
                            localizer.text("design-data-type"),
                            localizer.text("design-description"),
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            header.col(|ui| {
                                grid::cell(
                                    ui,
                                    &title,
                                    grid::CellStyle::Header {
                                        active: position == 0,
                                    },
                                );
                            });
                        }
                    })
                    .body(|body| {
                        body.rows(22.0, schema.fields.len(), |mut row| {
                            let position = row.index();
                            let field = &schema.fields[position];
                            row.col(|ui| {
                                if grid::cell(
                                    ui,
                                    &field.name,
                                    grid::CellStyle::FieldName {
                                        selected: position == self.selected_field,
                                    },
                                )
                                .clicked()
                                {
                                    self.selected_field = position;
                                    ui.ctx().request_repaint();
                                }
                            });
                            row.col(|ui| {
                                let fallback = crate::localization::tr(ui, "design-unsupported");
                                grid::cell(
                                    ui,
                                    field
                                        .display_type
                                        .as_deref()
                                        .or(field.scalar_type.as_deref())
                                        .unwrap_or(&fallback),
                                    grid::CellStyle::Value,
                                );
                            });
                            row.col(|ui| {
                                let description = field
                                    .properties
                                    .iter()
                                    .find_map(|property| match (&property.name, &property.value) {
                                        (&"Description", FieldPropertyValue::Text(value)) => {
                                            Some(value.as_str())
                                        }
                                        _ => None,
                                    })
                                    .unwrap_or("");
                                grid::cell(ui, description, grid::CellStyle::Value);
                            });
                        });
                    });
            });
        ui.separator();
        ui.label(RichText::new(crate::localization::tr(ui, "design-field-properties")).strong());
        let Some(field) = schema.fields.get(self.selected_field) else {
            return;
        };
        ScrollArea::vertical()
            .id_salt("field_properties")
            .show(ui, |ui| {
                properties::show(ui, field);
            });
    }
}

fn property(ui: &mut Ui, label: &str, value: impl Into<String>) {
    let name = ui.label(label);
    ui.add(egui::Label::new(value.into()).selectable(true))
        .labelled_by(name.id);
    ui.end_row();
}

fn yes_no(ui: &Ui, value: bool) -> String {
    crate::localization::tr(ui, if value { "shell-yes" } else { "shell-no" })
}

#[cfg(test)]
mod tests;
