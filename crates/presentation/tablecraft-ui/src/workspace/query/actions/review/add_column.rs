//! Actual captured scalar shape, without gallery defaults or native authority.
use crate::localization::{message, tr};
use crate::workspace::{field_addition::field_label_key, field_properties::number_size_label_key};
use egui::Ui;
use tablecraft_builders::table::TypedFieldAdditionDeclaration;

pub(super) fn show(ui: &mut Ui, column: &str, declaration: TypedFieldAdditionDeclaration) {
    pair(ui, "design-field-name", column);
    let kind_key = if declaration.short_text_units().is_some() {
        "design-type-short-text"
    } else {
        field_label_key(declaration.kind())
    };
    let kind = tr(ui, kind_key);
    pair(ui, "design-data-type", &kind);
    let descriptor = declaration.descriptor();
    if let Some(units) = declaration.short_text_units() {
        pair(ui, "design-field-size", &units.to_string());
    } else if let Some(key) = number_size_label_key(descriptor.scalar_type()) {
        let size = tr(ui, key);
        pair(ui, "design-field-size", &size);
    }
    if let Some(decimal) = descriptor.decimal_constraints() {
        let value = message(
            ui,
            "design-precision-value",
            &[
                ("precision", decimal.precision().to_string()),
                ("scale", decimal.scale().to_string()),
            ],
        );
        pair(ui, "design-precision-scale", &value);
    }
}

fn pair(ui: &mut Ui, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(tr(ui, key));
        ui.label(value);
    });
}

#[cfg(test)]
mod tests;
