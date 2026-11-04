//! Typed parameter entry displays runtime declarations without interpreting SQL.
use super::SavedQueryState;
use crate::{
    theme::{ACCENT, GRID_LINE},
    workspace::{Action, State},
};

pub(super) const INPUT_KINDS: &[(&str, &str)] = &[
    ("boolean", "Boolean"),
    ("byte", "Byte"),
    ("int16", "Integer (16-bit)"),
    ("int32", "Long integer (32-bit)"),
    ("int64", "Large integer (64-bit)"),
    ("single", "Single"),
    ("double", "Double"),
    ("currency", "Currency"),
    ("decimal", "Decimal"),
    ("text", "Text"),
    ("binary", "Binary"),
    ("guid", "GUID"),
    ("date", "Date/time"),
    ("datetime_extended", "Extended date/time"),
];

impl State {
    pub(in crate::workspace) fn saved_query_editor(
        &mut self,
        ui: &mut egui::Ui,
        actions: &mut Vec<Action>,
    ) {
        let Some(target) = &self.saved_query.target else {
            ui.weak("Choose a saved query in the object pane.");
            return;
        };
        ui.heading(&target.name);
        ui.small("Inspection does not execute the query. Run validates and executes it against the opened snapshot.");
        if self.saved_query.pending.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Working on the selected saved query…");
            });
        }
        let idle = !self.busy && self.saved_query.pending.is_none();
        let object_id = target.object_id;
        let changed = ui
            .push_id(("saved_parameters", object_id), |ui| {
                parameters(ui, &mut self.saved_query, idle)
            })
            .inner;
        if changed {
            self.saved_query.data = None;
            self.saved_query.selection.clear();
            self.saved_query.error = None;
        }
        if !super::inputs_within_limits(&self.saved_query.inputs) {
            ui.label("Parameter inputs exceed the 256-entry or 1 MiB input limit.");
        }
        if let Some(error) = &self.saved_query.error {
            ui.label(error);
        }
        let target = self
            .saved_query
            .target
            .as_ref()
            .expect("selected saved query");
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.can_run_saved_query(target, &self.saved_query.inputs),
                    egui::Button::new("Run saved query"),
                )
                .clicked()
                && let Some(action) = self.saved_query_run_action()
            {
                actions.push(action);
            }
            if ui
                .add_enabled(idle, egui::Button::new("Inspect again"))
                .clicked()
            {
                actions.push(Action::InspectSavedQuery {
                    target: target.clone(),
                });
            }
        });
        if self.saved_query.data.is_some()
            && (self.saved_query.pending.is_some() || self.saved_query.error.is_some())
        {
            ui.small("Previous successful results remain below until a complete run succeeds.");
        }
        ui.separator();
    }
}

fn parameters(ui: &mut egui::Ui, state: &mut SavedQueryState, idle: bool) -> bool {
    let Some(parameters) = &state.parameters else {
        return false;
    };
    let mut changed = false;
    egui::ScrollArea::vertical()
        .max_height(220.0)
        .show(ui, |ui| {
            for (index, (spec, input)) in parameters.iter().zip(&mut state.inputs).enumerate() {
                ui.push_id(index, |ui| {
                    let label = ui.label(format!("{} · {}", spec.name, spec.label));
                    ui.add_enabled_ui(idle, |ui| {
                        if spec.kind == "value" {
                            let kind = egui::ComboBox::from_id_salt("parameter_kind")
                                .selected_text(
                                    INPUT_KINDS
                                        .iter()
                                        .find(|(kind, _)| *kind == input.kind)
                                        .map_or("Choose a value type", |(_, label)| *label),
                                )
                                .show_ui(ui, |ui| {
                                    for (kind, label) in INPUT_KINDS {
                                        changed |= ui
                                            .selectable_value(
                                                &mut input.kind,
                                                (*kind).into(),
                                                *label,
                                            )
                                            .changed();
                                    }
                                })
                                .response
                                .labelled_by(label.id);
                            ui.ctx().accesskit_node_builder(kind.id, |node| {
                                node.set_description("Parameter value type");
                            });
                            input_border(ui, &kind);
                        }
                        let text = ui
                            .add_enabled(
                                !input.is_null,
                                egui::TextEdit::singleline(&mut input.text).desired_width(300.0),
                            )
                            .labelled_by(label.id);
                        input_border(ui, &text);
                        changed |= text.changed();
                        let null = ui
                            .add_enabled(
                                spec.nullable,
                                egui::Checkbox::new(&mut input.is_null, "NULL"),
                            )
                            .labelled_by(label.id);
                        ui.ctx().accesskit_node_builder(null.id, |node| {
                            node.set_description("Use NULL for this parameter");
                        });
                        changed |= null.changed();
                        ui.small(hint(&input.kind));
                    });
                    ui.add_space(4.0);
                });
            }
        });
    changed
}

fn input_border(ui: &egui::Ui, response: &egui::Response) {
    let color = if response.enabled() && response.has_focus() {
        ACCENT
    } else {
        GRID_LINE
    };
    ui.painter().rect_stroke(
        response.rect,
        2.0,
        egui::Stroke::new(1.0, color),
        egui::StrokeKind::Inside,
    );
}

fn hint(kind: &str) -> &'static str {
    match kind {
        "boolean" => "Enter true or false.",
        "binary" => "Hexadecimal byte pairs, without a 0x prefix.",
        "guid" => "Canonical GUID, such as 00112233-4455-6677-8899-aabbccddeeff.",
        "date" | "datetime_extended" => "ISO date and time, without a time zone.",
        "text" => "Literal text; do not add SQL quotation marks.",
        "" => "Choose a concrete type, including when supplying NULL.",
        "byte" | "int16" | "int32" | "int64" => {
            "Enter a whole number within the declared type's range."
        }
        _ => "Use a period (.) as the decimal separator.",
    }
}
