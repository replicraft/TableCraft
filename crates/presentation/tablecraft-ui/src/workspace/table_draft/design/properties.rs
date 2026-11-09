//! Editable draft declarations remain separate from stored metadata and literal parsing.
pub(in crate::workspace::table_draft) mod editor;
mod number;
mod sheet;
#[cfg(test)]
mod tests;

use super::Editor;
use crate::{localization::tr, theme::palette};
use editor::{DefaultChoice, MAX_SIZE_INPUT_BYTES};
use egui::{Ui, WidgetInfo, WidgetType};
use sheet::Sheet;
use tablecraft_builders::table::{DraftFieldType, DraftNumberSize, MAX_DRAFT_DEFAULT_BYTES};

const HELP_WIDTH_FRACTION: f32 = 0.28;
const HELP_DIVIDER_WIDTH: f32 = 1.0;
const PROPERTY_INPUT_MARGIN: egui::Margin = egui::Margin::symmetric(4, 2);

/// Keep the active working controls visible when locale font metrics or the
/// Decimal declaration adds rows. The caller still reserves the upper grid.
pub(super) fn minimum_height(ui: &Ui, editor: &Editor) -> f32 {
    if editor.new_field.editing() {
        return 0.0;
    }
    let field = editor
        .draft
        .fields()
        .iter()
        .find(|field| Some(field.id()) == editor.design.selected)
        .or_else(|| editor.draft.fields().first());
    let Some(field) = field else {
        return 0.0;
    };
    let font = egui::TextStyle::Body.resolve(ui.style());
    let line_height = |key| {
        let text_height = ui
            .painter()
            // The layout API consumes this FontId; keep the original style or loop-local
            // descriptor available for the next measurement.
            .layout_no_wrap(tr(ui, key), font.clone(), ui.visuals().text_color())
            .size()
            .y;
        // TextEdit and ComboBox add their vertical padding outside the galley;
        // interact_size alone can be shorter than a localized input's frame.
        let padding =
            f32::from(PROPERTY_INPUT_MARGIN.top) + f32::from(PROPERTY_INPUT_MARGIN.bottom);
        (text_height + padding.max(ui.spacing().button_padding.y * 2.0))
            .max(ui.spacing().interact_size.y)
    };
    let keys = [
        Some("design-field-properties"),
        Some("design-general"),
        Some("design-required"),
        Some("design-default-value"),
        (field.short_text_units().is_some()
            || field.number_size().is_some()
            || matches!(
                field.kind(),
                DraftFieldType::AutoNumberIncrement | DraftFieldType::LargeInteger
            ))
        .then_some("design-field-size"),
        field.short_text_units().map(|_| "design-allow-zero-length"),
        field
            .decimal_constraints()
            .map(|_| "table-draft-number-precision"),
        field
            .decimal_constraints()
            .map(|_| "table-draft-number-scale"),
        (DefaultChoice::from_mode(field.default_mode()) == DefaultChoice::Literal)
            .then_some("table-draft-property-literal-input"),
    ];
    let mut height = egui::Frame::side_top_panel(ui.style())
        .total_margin()
        .sum()
        .y;
    for key in keys.into_iter().flatten() {
        height += line_height(key) + ui.spacing().item_spacing.y;
    }
    height
}

pub(super) fn show(ui: &mut Ui, editor: &mut Editor) {
    if editor.new_field.editing() {
        ui.vertical_centered(|ui| ui.label(tr(ui, "design-field-properties")));
        return;
    }
    ui.vertical_centered(|ui| ui.label(tr(ui, "design-field-properties")));
    ui.horizontal(|ui| {
        for (lookup, key) in [(false, "design-general"), (true, "design-lookup")] {
            if ui
                .selectable_label(editor.design.lookup == lookup, tr(ui, key))
                .clicked()
            {
                editor.design.lookup = lookup;
            }
        }
    });
    let field = editor
        .draft
        .fields()
        .iter()
        .find(|field| Some(field.id()) == editor.design.selected)
        .or_else(|| editor.draft.fields().first());
    let Some(field) = field else {
        return;
    };
    let id = field.id();
    let Some(position) = editor.properties.ensure(&editor.draft, id) else {
        ui.colored_label(
            ui.visuals().error_fg_color,
            tr(ui, "table-draft-property-input-limit"),
        );
        return;
    };
    let available = ui.available_size();
    let help_width = available.x * HELP_WIDTH_FRACTION;
    let sheet_width =
        (available.x - help_width - ui.spacing().item_spacing.x * 2.0 - HELP_DIVIDER_WIDTH)
            .max(1.0);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(sheet_width, available.y),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                egui::ScrollArea::both()
                    .id_salt("initial_field_properties")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if editor.design.lookup {
                            ui.add_enabled(false, egui::Label::new(tr(ui, "ribbon-unavailable")));
                        } else {
                            show_general(ui, editor, position);
                        }
                    });
            },
        );
        let (divider, _) = ui.allocate_exact_size(
            egui::vec2(HELP_DIVIDER_WIDTH, available.y),
            egui::Sense::hover(),
        );
        ui.painter().vline(
            divider.center().x,
            divider.y_range(),
            egui::Stroke::new(HELP_DIVIDER_WIDTH, palette(ui.ctx()).grid_line),
        );
        ui.allocate_ui_with_layout(
            egui::vec2(help_width, available.y),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("initial_field_help")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.weak(tr(ui, "table-draft-property-help"));
                    });
            },
        );
    });
}

fn show_general(ui: &mut Ui, editor: &mut Editor, position: usize) {
    let selected = editor.design.selected;
    let field = editor
        .draft
        .fields()
        .iter()
        .find(|field| Some(field.id()) == selected)
        .or_else(|| editor.draft.fields().first())
        .unwrap();
    let field_id = field.id();
    let kind = field.kind();
    let protected = field.is_protected();
    let primary = field.is_primary();
    let number_size = field.number_size();
    let required = field.required();
    let zero_length = field.allow_zero_length();
    let mode = DefaultChoice::from_mode(field.default_mode());
    let mut next_mode = None;
    ui.push_id(("field_property_inputs", field_id), |ui| {
        let mut sheet = Sheet::new(ui);
        let value_width = sheet.value_width();
        egui::Grid::new("draft_field_facts")
            .spacing(egui::Vec2::ZERO)
            .min_row_height(sheet.row_height())
            .striped(false)
            .show(ui, |ui| {
                if kind == DraftFieldType::ShortText {
                    sheet.label(ui, "design-field-size", true);
                    let mut size = editor.properties.size(position).to_owned();
                    let id = ui.make_persistent_id("text_units");
                    let rejected = crate::text_input::reject_events_with_limits(
                        ui,
                        id,
                        &size,
                        MAX_SIZE_INPUT_BYTES,
                        MAX_SIZE_INPUT_BYTES,
                    );
                    let input = ui.add(
                        egui::TextEdit::singleline(&mut size)
                            .id(id)
                            .margin(PROPERTY_INPUT_MARGIN)
                            .desired_width(value_width),
                    );
                    let label = tr(ui, "design-field-size");
                    input.widget_info(|| {
                        WidgetInfo::labeled(WidgetType::TextEdit, ui.is_enabled(), &label)
                    });
                    if input.changed() || rejected {
                        editor
                            .properties
                            .set_size(&mut editor.draft, position, size, rejected);
                    }
                    if editor.properties.size_rejected(position) {
                        ui.ctx().accesskit_node_builder(input.id, |node| {
                            node.set_invalid(egui::accesskit::Invalid::True)
                        });
                        input.on_hover_text(tr(ui, "table-draft-property-invalid-size"));
                    }
                    sheet.end_row(ui);
                } else if let Some(size) = number_size {
                    number::show(ui, editor, position, size, primary, &mut sheet);
                } else if matches!(
                    kind,
                    DraftFieldType::AutoNumberIncrement | DraftFieldType::LargeInteger
                ) {
                    sheet.label(ui, "design-field-size", true);
                    ui.label(tr(
                        ui,
                        if kind == DraftFieldType::LargeInteger {
                            "design-size-large-number"
                        } else {
                            "design-size-long-integer"
                        },
                    ));
                    sheet.end_row(ui);
                }
                sheet.label(ui, "design-required", true);
                if let Some(value) = boolean_property(
                    ui,
                    "required",
                    "design-required",
                    required,
                    !protected,
                    value_width,
                ) {
                    editor
                        .properties
                        .set_required(&mut editor.draft, position, Some(value));
                }
                sheet.end_row(ui);
                if kind == DraftFieldType::ShortText {
                    sheet.label(ui, "design-allow-zero-length", true);
                    if let Some(value) = boolean_property(
                        ui,
                        "allow_zero_length",
                        "design-allow-zero-length",
                        zero_length,
                        !protected,
                        value_width,
                    ) {
                        editor
                            .properties
                            .set_zero_length(&mut editor.draft, position, Some(value));
                    }
                    sheet.end_row(ui);
                }
                sheet.label(ui, "design-default-value", true);
                ui.add_enabled_ui(!protected, |ui| {
                    let response = egui::ComboBox::from_id_salt("default_mode")
                        .width(value_width)
                        .selected_text(tr(ui, mode.key()))
                        .show_ui(ui, |ui| {
                            for choice in [
                                DefaultChoice::TypeDefault,
                                DefaultChoice::Absent,
                                DefaultChoice::Null,
                                DefaultChoice::Literal,
                            ] {
                                let available = choice != DefaultChoice::Literal
                                    || number_size != Some(DraftNumberSize::ReplicationId);
                                if ui
                                    .add_enabled(
                                        available,
                                        egui::Button::selectable(
                                            mode == choice,
                                            tr(ui, choice.key()),
                                        ),
                                    )
                                    .on_disabled_hover_text(tr(ui, "ribbon-unavailable"))
                                    .clicked()
                                {
                                    next_mode = Some(choice);
                                    ui.close();
                                }
                            }
                        })
                        .response;
                    let label = tr(ui, "design-default-value");
                    response.widget_info(|| {
                        WidgetInfo::labeled(WidgetType::ComboBox, response.enabled(), &label)
                    });
                });
                sheet.end_row(ui);
                if mode == DefaultChoice::Literal {
                    if number_size == Some(DraftNumberSize::ReplicationId) {
                        sheet.empty_label(ui);
                        ui.add_sized(
                            [value_width, ui.spacing().interact_size.y],
                            egui::Label::new(
                                egui::RichText::new(tr(ui, "table-draft-number-guid-default"))
                                    .color(ui.visuals().error_fg_color),
                            )
                            .wrap(),
                        );
                        sheet.end_row(ui);
                    }
                    sheet.label(ui, "table-draft-property-literal-input", true);
                    let mut literal = editor.properties.literal(position).to_owned();
                    let id = ui.make_persistent_id("literal_default");
                    let rejected = crate::text_input::reject_events_with_limits(
                        ui,
                        id,
                        &literal,
                        MAX_DRAFT_DEFAULT_BYTES,
                        MAX_DRAFT_DEFAULT_BYTES,
                    );
                    let input = ui.add_enabled(
                        !protected,
                        egui::TextEdit::multiline(&mut literal)
                            .desired_rows(1)
                            .id(id)
                            .margin(PROPERTY_INPUT_MARGIN)
                            .desired_width(value_width),
                    );
                    let label = tr(ui, "table-draft-property-literal-input");
                    // Keep TextEdit's actual multiline role and text actions.
                    // A generic WidgetType::TextEdit annotation would reset it.
                    ui.ctx()
                        .accesskit_node_builder(input.id, |node| node.set_label(label));
                    if input.changed() || rejected {
                        editor.properties.set_literal(
                            &mut editor.draft,
                            position,
                            literal,
                            rejected,
                        );
                    }
                    if editor.properties.literal_rejected(position) {
                        ui.ctx().accesskit_node_builder(input.id, |node| {
                            node.set_invalid(egui::accesskit::Invalid::True)
                        });
                        input.on_hover_text(tr(ui, "table-draft-property-input-limit"));
                    }
                    sheet.end_row(ui);
                }
                for key in [
                    "design-caption",
                    "design-validation-rule",
                    "design-validation-text",
                ] {
                    sheet.label(ui, key, false);
                    ui.add_enabled(false, egui::Label::new(tr(ui, "ribbon-unavailable")));
                    sheet.end_row(ui);
                }
            });
        sheet.paint(ui);
    });
    if let Some(mode) = next_mode {
        editor
            .properties
            .set_mode(&mut editor.draft, position, mode);
    }
    if editor.properties.size_rejected(position) {
        ui.colored_label(
            ui.visuals().error_fg_color,
            tr(ui, "table-draft-property-invalid-size"),
        );
    }
    if editor.properties.literal_rejected(position) {
        ui.colored_label(
            ui.visuals().error_fg_color,
            tr(ui, "table-draft-property-input-limit"),
        );
    }
}

fn boolean_property(
    ui: &mut Ui,
    id: &str,
    label: &str,
    current: Option<bool>,
    enabled: bool,
    value_width: f32,
) -> Option<bool> {
    let mut selected = None;
    ui.add_enabled_ui(enabled, |ui| {
        let response = egui::ComboBox::from_id_salt(id)
            .width(value_width)
            .selected_text(tr(ui, flag_key(current)))
            .show_ui(ui, |ui| {
                for value in [false, true] {
                    if ui
                        .selectable_label(current == Some(value), tr(ui, flag_key(Some(value))))
                        .clicked()
                    {
                        selected = Some(value);
                        ui.close();
                    }
                }
            })
            .response;
        let accessible_label = tr(ui, label);
        response.widget_info(|| {
            WidgetInfo::labeled(WidgetType::ComboBox, response.enabled(), &accessible_label)
        });
    });
    selected
}

fn flag_key(value: Option<bool>) -> &'static str {
    match value {
        None => "table-draft-property-unset",
        Some(false) => "table-draft-property-no",
        Some(true) => "table-draft-property-yes",
    }
}
