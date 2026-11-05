//! Typed parameter entry displays runtime declarations without interpreting SQL.
use super::SavedQueryState;
use crate::localization::{message, tr};
use crate::theme::input_border;
use crate::workspace::focus::{self, Pane};
use crate::workspace::{Action, State, diagnostics};

const MAX_PARAMETER_PANE_HEIGHT: f32 = 220.0;
const MIN_PARAMETER_PANE_HEIGHT: f32 = 24.0;
const MIN_PREVIOUS_RESULTS_HEIGHT: f32 = 60.0;

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
            ui.weak(tr(ui, "saved-query-choose"));
            return;
        };
        ui.heading(&target.name);
        ui.small(tr(ui, "saved-query-help"));
        if self.saved_query.pending.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(tr(ui, "saved-query-working"));
            });
        }
        let idle = !self.busy && self.saved_query.pending.is_none();
        let object_id = target.object_id;
        let document_revision = target.document_revision;
        if let Some(error) = &self.saved_query.error {
            let id = ui.make_persistent_id(("saved_query_error", document_revision, object_id));
            diagnostics::show(ui, id, error);
        }
        let parameter_height = if self.saved_query.error.is_some() {
            // Measure after the complete diagnostic and its Copy command, then
            // leave room for Run/Inspect, status and at least one result row.
            let reserved = ui.spacing().interact_size.y
                + ui.text_style_height(&egui::TextStyle::Small)
                + ui.spacing().item_spacing.y * 5.0
                + MIN_PREVIOUS_RESULTS_HEIGHT;
            (ui.available_height() - reserved)
                .clamp(MIN_PARAMETER_PANE_HEIGHT, MAX_PARAMETER_PANE_HEIGHT)
        } else {
            MAX_PARAMETER_PANE_HEIGHT
        };
        let changed = ui
            .push_id(("saved_parameters", document_revision, object_id), |ui| {
                parameters(ui, &mut self.saved_query, idle, parameter_height)
            })
            .inner;
        if changed {
            self.saved_query.data = None;
            self.saved_query.selection.clear();
            self.saved_query.error = None;
        }
        if !super::inputs_within_limits(&self.saved_query.inputs) {
            ui.label(tr(ui, "saved-query-input-limit"));
        }
        let target = self
            .saved_query
            .target
            .as_ref()
            .expect("selected saved query");
        ui.horizontal(|ui| {
            let run = ui.add_enabled(
                self.can_run_saved_query(target, &self.saved_query.inputs),
                egui::Button::new(tr(ui, "saved-query-run")),
            );
            focus::register(ui, Pane::Document, &run, false);
            if run.clicked()
                && let Some(action) = self.saved_query_run_action()
            {
                actions.push(action);
            }
            let inspect = ui.add_enabled(idle, egui::Button::new(tr(ui, "saved-query-inspect")));
            focus::register(ui, Pane::Document, &inspect, false);
            if inspect.clicked() {
                actions.push(Action::InspectSavedQuery {
                    target: target.clone(),
                });
            }
        });
        if self.saved_query.data.is_some()
            && (self.saved_query.pending.is_some() || self.saved_query.error.is_some())
        {
            ui.small(tr(ui, "saved-query-previous-results"));
        }
        ui.separator();
    }
}

fn parameters(ui: &mut egui::Ui, state: &mut SavedQueryState, idle: bool, max_height: f32) -> bool {
    let Some(parameters) = &state.parameters else {
        return false;
    };
    ui.scope(|ui| {
        ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
        let mut changed = false;
        egui::ScrollArea::vertical()
            .id_salt("saved_query_parameters")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .max_height(max_height)
            .show(ui, |ui| {
                for (index, (spec, input)) in parameters.iter().zip(&mut state.inputs).enumerate() {
                    ui.push_id(index, |ui| {
                        let label = ui.label(message(
                            ui,
                            "saved-query-parameter-label",
                            &[("name", spec.name.clone()), ("kind", spec.label.clone())],
                        ));
                        ui.add_enabled_ui(idle, |ui| {
                            if spec.kind == "value" {
                                let kind = egui::ComboBox::from_id_salt("parameter_kind")
                                    .selected_text(
                                        INPUT_KINDS
                                            .iter()
                                            .find(|(kind, _)| *kind == input.kind)
                                            .map_or_else(
                                                || tr(ui, "saved-query-choose-type"),
                                                |(kind, _)| type_label(ui, kind),
                                            ),
                                    )
                                    .show_ui(ui, |ui| {
                                        for (kind, _) in INPUT_KINDS {
                                            changed |= ui
                                                .selectable_value(
                                                    &mut input.kind,
                                                    (*kind).into(),
                                                    type_label(ui, kind),
                                                )
                                                .changed();
                                        }
                                    })
                                    .response
                                    .labelled_by(label.id);
                                let description = tr(ui, "saved-query-value-type");
                                ui.ctx().accesskit_node_builder(kind.id, |node| {
                                    node.set_description(description);
                                });
                                input_border(ui, &kind);
                                focus::register(
                                    ui,
                                    Pane::Document,
                                    &kind,
                                    kind.has_focus() || index == 0 && input.kind.is_empty(),
                                );
                                if kind.gained_focus() {
                                    kind.scroll_to_me(None);
                                }
                            }
                            let text = ui
                                .add_enabled(
                                    !input.is_null,
                                    egui::TextEdit::singleline(&mut input.text)
                                        .desired_width(300.0),
                                )
                                .labelled_by(label.id);
                            input_border(ui, &text);
                            focus::register(
                                ui,
                                Pane::Document,
                                &text,
                                text.has_focus()
                                    || index == 0
                                        && (spec.kind != "value" || !input.kind.is_empty()),
                            );
                            if text.gained_focus() {
                                text.scroll_to_me(None);
                            }
                            changed |= text.changed();
                            let null = ui
                                .add_enabled(
                                    spec.nullable,
                                    egui::Checkbox::new(
                                        &mut input.is_null,
                                        tr(ui, "saved-query-null"),
                                    ),
                                )
                                .labelled_by(label.id);
                            focus::register(
                                ui,
                                Pane::Document,
                                &null,
                                null.has_focus() || index == 0 && input.is_null,
                            );
                            let description = tr(ui, "saved-query-use-null");
                            ui.ctx().accesskit_node_builder(null.id, |node| {
                                node.set_description(description);
                            });
                            if null.gained_focus() {
                                null.scroll_to_me(None);
                            }
                            changed |= null.changed();
                            ui.small(hint(ui, &input.kind));
                        });
                        ui.add_space(4.0);
                    });
                }
            });
        changed
    })
    .inner
}

fn type_label(ui: &egui::Ui, kind: &str) -> String {
    let suffix = if kind == "datetime_extended" {
        "datetime-extended"
    } else {
        kind
    };
    tr(ui, &format!("saved-query-type-{suffix}"))
}

fn hint(ui: &egui::Ui, kind: &str) -> String {
    let key = match kind {
        "boolean" => "saved-query-boolean-help",
        "binary" => "saved-query-binary-help",
        "guid" => "saved-query-guid-help",
        "date" | "datetime_extended" => "saved-query-date-help",
        "text" => "saved-query-text-help",
        "" => "saved-query-value-help",
        "byte" | "int16" | "int32" | "int64" => "saved-query-integer-help",
        _ => "saved-query-number-help",
    };
    tr(ui, key)
}
