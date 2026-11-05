//! Inert field-property observations, distinct from native storage diagnostics.
use super::{Field, property, yes_no};
use crate::Localizer;
use egui::{RichText, Ui};

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
    ui.label(RichText::new(localizer.text("design-general")).strong());
    if field.properties.is_empty() && field.field_size.is_none() {
        ui.weak(localizer.text("design-no-observations"));
    } else {
        egui::Grid::new("general_field_properties")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                if let Some(size) = &field.field_size {
                    property(ui, &localizer.text("design-field-size"), size);
                }
                for observation in &field.properties {
                    let value = match &observation.value {
                        FieldPropertyValue::Uninspected => localizer.text("design-not-inspected"),
                        FieldPropertyValue::Absent => localizer.text("design-not-stored"),
                        FieldPropertyValue::Boolean(value) => yes_no(ui, *value),
                        FieldPropertyValue::Text(value) => value.clone(),
                        FieldPropertyValue::Unsupported => {
                            localizer.text("design-unsupported-value")
                        }
                    };
                    property(ui, &observation.label(&localizer), value);
                }
            });
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
