//! Editable draft declarations remain separate from stored metadata and literal parsing.
pub(in crate::workspace::table_draft) mod editor;
#[cfg(test)]
mod tests;

use super::Editor;
use crate::localization::tr;
use editor::{DefaultChoice, MAX_SIZE_INPUT_BYTES};
use egui::{Ui, WidgetInfo, WidgetType};
use tablecraft_builders::table::{DraftFieldType, MAX_DRAFT_DEFAULT_BYTES};

const HELP_WIDTH_FRACTION: f32 = 0.28;
const PROPERTY_INPUT_WIDTH: f32 = 180.0;

pub(super) fn show(ui: &mut Ui, editor: &mut Editor) {
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
    let sheet_width = (available.x - help_width - ui.spacing().item_spacing.x).max(1.0);
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
    let primary = field.is_primary();
    let required = field.required();
    let zero_length = field.allow_zero_length();
    let mode = DefaultChoice::from_mode(field.default_mode());
    let mut next_mode = None;
    ui.push_id(("field_property_inputs", field_id), |ui| {
        egui::Grid::new("draft_field_facts")
            .striped(false)
            .show(ui, |ui| {
                if kind == DraftFieldType::ShortText {
                    ui.label(tr(ui, "design-field-size"));
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
                            .desired_width(PROPERTY_INPUT_WIDTH),
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
                    ui.end_row();
                } else if matches!(
                    kind,
                    DraftFieldType::AutoNumberIncrement
                        | DraftFieldType::LongInteger
                        | DraftFieldType::LargeInteger
                ) {
                    ui.label(tr(ui, "design-field-size"));
                    ui.label(tr(
                        ui,
                        if kind == DraftFieldType::LargeInteger {
                            "design-size-large-number"
                        } else {
                            "design-size-long-integer"
                        },
                    ));
                    ui.end_row();
                }
                ui.label(tr(ui, "design-required"));
                if let Some(value) =
                    boolean_property(ui, "required", "design-required", required, !primary)
                {
                    editor
                        .properties
                        .set_required(&mut editor.draft, position, Some(value));
                }
                ui.end_row();
                if kind == DraftFieldType::ShortText {
                    ui.label(tr(ui, "design-allow-zero-length"));
                    if let Some(value) = boolean_property(
                        ui,
                        "allow_zero_length",
                        "design-allow-zero-length",
                        zero_length,
                        !primary,
                    ) {
                        editor
                            .properties
                            .set_zero_length(&mut editor.draft, position, Some(value));
                    }
                    ui.end_row();
                }
                ui.label(tr(ui, "design-default-value"));
                ui.add_enabled_ui(!primary, |ui| {
                    let response = egui::ComboBox::from_id_salt("default_mode")
                        .width(PROPERTY_INPUT_WIDTH)
                        .selected_text(tr(ui, mode.key()))
                        .show_ui(ui, |ui| {
                            for choice in [
                                DefaultChoice::TypeDefault,
                                DefaultChoice::Absent,
                                DefaultChoice::Null,
                                DefaultChoice::Literal,
                            ] {
                                if ui
                                    .selectable_label(mode == choice, tr(ui, choice.key()))
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
                ui.end_row();
                if mode == DefaultChoice::Literal {
                    ui.label(tr(ui, "table-draft-property-literal-input"));
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
                        !primary,
                        egui::TextEdit::multiline(&mut literal)
                            .desired_rows(1)
                            .id(id)
                            .desired_width(PROPERTY_INPUT_WIDTH),
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
                    ui.end_row();
                }
                for key in [
                    "design-caption",
                    "design-validation-rule",
                    "design-validation-text",
                ] {
                    ui.add_enabled(false, egui::Label::new(tr(ui, key)));
                    ui.add_enabled(false, egui::Label::new(tr(ui, "ribbon-unavailable")));
                    ui.end_row();
                }
            });
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
) -> Option<bool> {
    let mut selected = None;
    ui.add_enabled_ui(enabled, |ui| {
        let response = egui::ComboBox::from_id_salt(id)
            .width(PROPERTY_INPUT_WIDTH)
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
