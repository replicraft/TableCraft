//! Draft facts and unavailable property editing remain separate from stored metadata.
use super::{Editor, type_key};
use crate::localization::tr;
use egui::Ui;
use tablecraft_builders::table::{DraftFieldType, SHORT_TEXT_UNITS};

const HELP_WIDTH_FRACTION: f32 = 0.28;

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
                            return;
                        }
                        egui::Grid::new("draft_field_facts")
                            .striped(false)
                            .show(ui, |ui| {
                                ui.label(tr(ui, "design-field-name"));
                                ui.label(field.name());
                                ui.end_row();
                                ui.label(tr(ui, "design-data-type"));
                                ui.label(tr(ui, type_key(field.kind())));
                                ui.end_row();
                                let size = match field.kind() {
                                    DraftFieldType::AutoNumberIncrement
                                    | DraftFieldType::LongInteger => {
                                        Some(tr(ui, "design-size-long-integer"))
                                    }
                                    DraftFieldType::LargeInteger => {
                                        Some(tr(ui, "design-size-large-number"))
                                    }
                                    DraftFieldType::ShortText => Some(SHORT_TEXT_UNITS.to_string()),
                                    _ => None,
                                };
                                if let Some(size) = size {
                                    ui.label(tr(ui, "design-field-size"));
                                    ui.label(size);
                                    ui.end_row();
                                }
                                for key in [
                                    "design-default-value",
                                    "design-required",
                                    "design-caption",
                                    "design-validation-rule",
                                    "design-validation-text",
                                ] {
                                    ui.add_enabled(false, egui::Label::new(tr(ui, key)));
                                    ui.add_enabled(
                                        false,
                                        egui::Label::new(tr(ui, "ribbon-unavailable")),
                                    );
                                    ui.end_row();
                                }
                            });
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
                        ui.weak(tr(ui, "table-draft-design-help"));
                    });
            },
        );
    });
}
