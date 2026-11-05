//! Read-only Table Design uses host-supplied facts without interpreting storage.
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
    pub reader_nullable: Option<bool>,
    pub decimal_parameters: Option<(u8, u8)>,
    pub complex_type_id: Option<u32>,
    pub properties: Vec<FieldProperty>,
}

impl State {
    pub(super) fn design(&mut self, ui: &mut Ui) {
        let Some(schema) = &self.schema else {
            ui.weak("Choose View schema for a table to inspect its declaration.");
            return;
        };
        ui.horizontal(|ui| {
            ui.heading(&schema.table);
            ui.label("Table Design · read only");
        });
        ui.small(format!(
            "Declared rows (unverified): {}",
            schema.declared_rows
        ));
        ui.small("Stored declarations only. Defaults and validation rules are displayed without evaluation.");
        ui.separator();
        if schema.fields.is_empty() {
            ui.weak("No fields in the captured declaration.");
            return;
        }

        egui::Resize::default()
            .id_salt("schema_fields")
            .default_height(250.0)
            .min_height(90.0)
            .max_height((ui.available_height() - 140.0).max(90.0))
            .resizable([false, true])
            .show(ui, |ui| {
                TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .column(Column::initial(210.0).at_least(100.0))
                    .column(Column::initial(140.0).at_least(90.0))
                    .column(Column::initial(160.0).at_least(100.0))
                    .column(Column::remainder().at_least(70.0))
                    .header(22.0, |mut header| {
                        for title in [
                            "Field Name",
                            "Declared Native Type",
                            "Readable Carrier",
                            "Storage Bytes",
                        ] {
                            header.col(|ui| {
                                ui.label(title);
                            });
                        }
                    })
                    .body(|body| {
                        body.rows(22.0, schema.fields.len(), |mut row| {
                            let position = row.index();
                            let field = &schema.fields[position];
                            row.set_selected(position == self.selected_field);
                            row.col(|ui| {
                                if ui
                                    .selectable_label(position == self.selected_field, &field.name)
                                    .clicked()
                                {
                                    self.selected_field = position;
                                    ui.ctx().request_repaint();
                                }
                            });
                            row.col(|ui| {
                                ui.label(format!("0x{:02X}", field.native_type));
                            });
                            row.col(|ui| {
                                ui.label(field.scalar_type.as_deref().unwrap_or("Unsupported"));
                            });
                            row.col(|ui| {
                                ui.label(field.storage_bytes.to_string());
                            });
                        });
                    });
            });
        ui.separator();
        ui.label(RichText::new("Field Properties").strong());
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

fn yes_no(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}

#[cfg(test)]
mod tests;
