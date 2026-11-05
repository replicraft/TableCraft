//! Inert field-property observations, distinct from native storage diagnostics.
use super::{Field, property, yes_no};
use egui::{RichText, Ui};

/// A named observation from the current metadata inspection.
pub struct FieldProperty {
    pub name: &'static str,
    pub value: FieldPropertyValue,
}

impl FieldProperty {
    /// Presentation labels do not replace the canonical native property key.
    pub fn label(&self) -> &'static str {
        match self.name {
            "AllowZeroLength" => "Allow Zero Length",
            "DefaultValue" => "Default Value",
            "ValidationRule" => "Validation Rule",
            "ValidationText" => "Validation Text",
            name => name,
        }
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
    ui.label(RichText::new("General").strong());
    if field.properties.is_empty() {
        ui.weak("No property observations supplied.");
    } else {
        egui::Grid::new("general_field_properties")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                for observation in &field.properties {
                    let value = match &observation.value {
                        FieldPropertyValue::Uninspected => "Not inspected",
                        FieldPropertyValue::Absent => "Not stored",
                        FieldPropertyValue::Boolean(value) => yes_no(*value),
                        FieldPropertyValue::Text(value) => value,
                        FieldPropertyValue::Unsupported => "Unsupported value",
                    };
                    property(ui, observation.label(), value);
                }
            });
    }

    ui.add_space(6.0);
    egui::CollapsingHeader::new("Native details").show(ui, |ui| {
        egui::Grid::new("native_field_properties")
            .num_columns(2)
            .show(ui, |ui| {
                property(ui, "Field name", &field.name);
                property(ui, "Native field number", field.number.to_string());
                property(ui, "Fixed storage", yes_no(field.fixed));
                property(ui, "Native flags", format!("0x{:02X}", field.flags));
                property(
                    ui,
                    "Extended flags",
                    format!("0x{:02X}", field.extended_flags),
                );
                property(
                    ui,
                    "Reader NULL support",
                    field.reader_nullable.map(yes_no).unwrap_or("Unclassified"),
                );
                if let Some((precision, scale)) = field.decimal_parameters {
                    property(ui, "Precision / scale", format!("{precision} / {scale}"));
                }
                if let Some(id) = field.complex_type_id {
                    property(ui, "Complex type ID", id.to_string());
                }
            });
        ui.small(
            "Reader NULL support describes the read carrier, not the declared Required property.",
        );
    });
}
