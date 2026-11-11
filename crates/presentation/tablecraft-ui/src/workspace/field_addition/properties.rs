//! One source-bound field declaration, with raw editors separate from valid facts.
use super::{Draft, FieldAdditionChoices, Phase};
use crate::{
    localization::tr,
    workspace::field_properties::{self, sheet::Sheet},
};
use egui::Ui;
use std::num::NonZeroU8;
use tablecraft_builders::table::{
    DraftNumberSize, FieldAdditionDeclaration, FieldAdditionKind, TypedFieldAdditionDeclaration,
};

pub(super) struct Properties {
    number: DraftNumberSize,
    precision: u8,
    scale: u8,
    precision_text: String,
    scale_text: String,
    units: NonZeroU8,
    units_text: String,
    compression: bool,
    decimal_invalid: bool,
    size_invalid: bool,
    explicit_number: bool,
    explicit_text: bool,
}

impl Properties {
    pub(super) fn new(declaration: FieldAdditionDeclaration) -> Self {
        let decimal = DraftNumberSize::initial_decimal_constraints();
        let mut result = Self {
            number: DraftNumberSize::LongInteger,
            precision: decimal.precision(),
            scale: decimal.scale(),
            precision_text: decimal.precision().to_string(),
            scale_text: decimal.scale().to_string(),
            units: NonZeroU8::MAX,
            units_text: "255".into(),
            compression: true,
            decimal_invalid: false,
            size_invalid: false,
            explicit_number: false,
            explicit_text: false,
        };
        match declaration {
            FieldAdditionDeclaration::NumberLongZero => {}
            FieldAdditionDeclaration::ShortText {
                max_utf16_units,
                unicode_compression,
            } => {
                result.units = max_utf16_units;
                result.units_text = max_utf16_units.to_string();
                result.compression = unicode_compression;
            }
            FieldAdditionDeclaration::Typed(value) => {
                if let Some(number) =
                    field_properties::number_size_for_type(value.descriptor().scalar_type())
                {
                    result.number = number;
                    result.explicit_number = true;
                }
                if let Some(decimal) = value.descriptor().decimal_constraints() {
                    result.precision = decimal.precision();
                    result.scale = decimal.scale();
                    result.precision_text = result.precision.to_string();
                    result.scale_text = result.scale.to_string();
                }
                if let Some(units) = value.short_text_units() {
                    result.units = units;
                    result.units_text = units.to_string();
                    result.compression = value.unicode_compression();
                    result.explicit_text = true;
                }
            }
        }
        result
    }

    pub(super) fn retained_bytes(&self) -> usize {
        self.precision_text.capacity() + self.scale_text.capacity() + self.units_text.capacity()
    }

    pub(super) fn invalid(&self) -> bool {
        self.decimal_invalid || self.size_invalid
    }

    pub(super) fn declaration(
        &self,
        kind: FieldAdditionKind,
        choices: &FieldAdditionChoices,
    ) -> Option<FieldAdditionDeclaration> {
        Some(match kind {
            FieldAdditionKind::Number
                if self.explicit_number
                    && self.number == DraftNumberSize::LongInteger
                    && choices.number
                    && !choices.typed_declarations.iter().any(|fact| {
                        fact.declaration.descriptor().scalar_type() == self.number.scalar_type()
                    }) =>
            {
                FieldAdditionDeclaration::NumberLongZero
            }
            FieldAdditionKind::Number
                if self.explicit_number && choices.supports_scalar(self.number.scalar_type()) =>
            {
                let value = if self.number == DraftNumberSize::Decimal {
                    TypedFieldAdditionDeclaration::decimal(self.precision, self.scale).ok()?
                } else {
                    TypedFieldAdditionDeclaration::fixed(self.number.scalar_type()).ok()?
                };
                FieldAdditionDeclaration::Typed(value)
            }
            FieldAdditionKind::ShortText
                if self.explicit_text
                    && choices
                        .typed_declarations
                        .iter()
                        .any(|fact| fact.declaration.kind() == kind) =>
            {
                FieldAdditionDeclaration::Typed(TypedFieldAdditionDeclaration::short_text(
                    self.units,
                    self.compression,
                ))
            }
            FieldAdditionKind::ShortText => FieldAdditionDeclaration::ShortText {
                max_utf16_units: self.units,
                unicode_compression: self.compression,
            },
            _ => choices.default_declaration(kind)?,
        })
    }
}

fn show(ui: &mut Ui, draft: &mut Draft, enabled: bool, choices: &FieldAdditionChoices) {
    let enabled =
        enabled && draft.phase == Phase::Naming && draft.revision.checked_add(1).is_some();
    let kind = draft.declaration.kind();
    ui.horizontal(|ui| {
        let _ = ui
            .selectable_label(true, tr(ui, "design-general"))
            .on_hover_text(tr(ui, "design-field-properties"));
        ui.add_enabled(false, egui::Button::new(tr(ui, "design-lookup")))
            .on_disabled_hover_text(tr(ui, "ribbon-unavailable"));
    });
    let mut changed = false;
    ui.add_enabled_ui(enabled, |ui| {
        let mut sheet = Sheet::new(ui);
        let width = sheet.value_width();
        egui::Grid::new("saved_field_draft_properties")
            .spacing(egui::Vec2::ZERO)
            .min_row_height(sheet.row_height())
            .show(ui, |ui| {
                if kind == FieldAdditionKind::Number {
                    sheet.label(ui, "design-field-size", true);
                    if let Some(size) = field_properties::number_size(
                        ui,
                        draft.properties.number,
                        |size| choices.supports_scalar(size.scalar_type()),
                        width,
                    ) {
                        draft.properties.number = size;
                        if draft.properties.decimal_invalid {
                            draft.properties.precision_text =
                                draft.properties.precision.to_string();
                            draft.properties.scale_text = draft.properties.scale.to_string();
                            draft.properties.decimal_invalid = false;
                        }
                        draft.properties.explicit_number = true;
                        changed = true;
                    }
                    sheet.end_row(ui);
                    if draft.properties.number == DraftNumberSize::Decimal {
                        for (precision, key) in [
                            (true, "table-draft-number-precision"),
                            (false, "table-draft-number-scale"),
                        ] {
                            sheet.label(ui, key, true);
                            let text = if precision {
                                &mut draft.properties.precision_text
                            } else {
                                &mut draft.properties.scale_text
                            };
                            let (response, rejected) =
                                field_properties::unsigned_input(ui, key, key, text, width);
                            if response.changed() || rejected {
                                let values = draft
                                    .properties
                                    .precision_text
                                    .parse::<u8>()
                                    .ok()
                                    .zip(draft.properties.scale_text.parse::<u8>().ok());
                                let valid = values.and_then(|(p, s)| {
                                    TypedFieldAdditionDeclaration::decimal(p, s)
                                        .ok()
                                        .map(|_| (p, s))
                                });
                                draft.properties.decimal_invalid = rejected || valid.is_none();
                                if let Some((p, s)) = valid.filter(|_| !rejected) {
                                    draft.properties.precision = p;
                                    draft.properties.scale = s;
                                }
                                changed = true;
                            }
                            if draft.properties.decimal_invalid {
                                ui.ctx().accesskit_node_builder(response.id, |node| {
                                    node.set_invalid(egui::accesskit::Invalid::True)
                                });
                                response
                                    .on_hover_text(tr(ui, "table-draft-number-invalid-decimal"));
                            }
                            sheet.end_row(ui);
                        }
                    }
                } else if kind == FieldAdditionKind::ShortText {
                    sheet.label(ui, "design-field-size", true);
                    let (response, rejected) = field_properties::unsigned_input(
                        ui,
                        "text_units",
                        "design-field-size",
                        &mut draft.properties.units_text,
                        width,
                    );
                    if response.changed() || rejected {
                        let value = draft
                            .properties
                            .units_text
                            .parse::<u8>()
                            .ok()
                            .and_then(NonZeroU8::new);
                        draft.properties.size_invalid = rejected || value.is_none();
                        if let Some(value) = value.filter(|_| !rejected) {
                            draft.properties.units = value;
                        }
                        draft.properties.explicit_text = true;
                        changed = true;
                    }
                    if draft.properties.size_invalid {
                        ui.ctx().accesskit_node_builder(response.id, |node| {
                            node.set_invalid(egui::accesskit::Invalid::True)
                        });
                        response.on_hover_text(tr(ui, "table-draft-property-invalid-size"));
                    }
                    sheet.end_row(ui);
                    sheet.label(ui, "design-unicode-compression", true);
                    if let Some(value) = field_properties::boolean_property(
                        ui,
                        "unicode_compression",
                        "design-unicode-compression",
                        Some(draft.properties.compression),
                        true,
                        width,
                    ) {
                        draft.properties.compression = value;
                        draft.properties.explicit_text = true;
                        changed = true;
                    }
                    sheet.end_row(ui);
                } else if kind == FieldAdditionKind::LargeInteger {
                    sheet.label(ui, "design-field-size", true);
                    ui.label(tr(ui, "design-size-large-number"));
                    sheet.end_row(ui);
                }
                sheet.label(ui, "design-required", false);
                field_properties::boolean_property(
                    ui,
                    "required",
                    "design-required",
                    Some(false),
                    false,
                    width,
                );
                sheet.end_row(ui);
                if kind == FieldAdditionKind::ShortText {
                    sheet.label(ui, "design-allow-zero-length", false);
                    field_properties::boolean_property(
                        ui,
                        "allow_zero_length",
                        "design-allow-zero-length",
                        Some(true),
                        false,
                        width,
                    );
                    sheet.end_row(ui);
                }
                sheet.label(ui, "design-default-value", false);
                ui.add_enabled(
                    false,
                    egui::Button::new(tr(ui, "table-draft-property-type-default")),
                )
                .on_disabled_hover_text(tr(ui, "ribbon-unavailable"));
                sheet.end_row(ui);
            });
        sheet.paint(ui);
    });
    if changed && let Some(revision) = draft.revision.checked_add(1) {
        if let Some(declaration) = draft.properties.declaration(kind, choices) {
            draft.declaration = declaration;
        }
        draft.revision = revision;
    }
    if draft.properties.invalid() {
        ui.colored_label(
            ui.visuals().error_fg_color,
            tr(ui, "table-draft-property-invalid"),
        );
    }
    ui.weak(tr(ui, "datasheet-field-type-default-help"));
}

impl crate::State {
    pub(in crate::workspace) fn show_field_addition_properties(
        &mut self,
        ui: &mut Ui,
        enabled: bool,
        choices: &FieldAdditionChoices,
    ) {
        if let Some(draft) = &mut self.field_addition.draft {
            egui::ScrollArea::vertical()
                .id_salt("saved_field_properties_scroll")
                .show(ui, |ui| show(ui, draft, enabled, choices));
        }
    }
}

#[cfg(test)]
mod tests;
