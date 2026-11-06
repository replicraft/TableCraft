//! Typed parameter entry displays runtime declarations without interpreting SQL.
use super::{ParameterInput, ParameterSpec};
use crate::localization::tr;
use crate::theme::input_border;
use crate::workspace::focus::{self, Pane};
use crate::workspace::{Action, State, diagnostics};

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
        if !super::inputs_within_limits(&self.saved_query.inputs) {
            ui.small(tr(ui, "saved-query-input-limit"));
        }
        let target = self
            .saved_query
            .target
            .as_ref()
            .expect("selected saved query")
            .clone();
        ui.horizontal(|ui| {
            let run = ui.add_enabled(
                self.saved_query_command_ready(),
                egui::Button::new(tr(ui, "saved-query-run")),
            );
            focus::register(ui, Pane::Document, &run, false);
            if run.clicked()
                && let Some(action) = self.request_saved_query_run()
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

pub(super) fn parameters(
    ui: &mut egui::Ui,
    parameters: &[ParameterSpec],
    inputs: &mut [ParameterInput],
    max_height: f32,
    focus_input: bool,
    max_text_bytes: usize,
) -> ParameterResponse {
    ui.scope(|ui| {
        ui.spacing_mut().scroll = egui::style::ScrollStyle::solid();
        let mut result = ParameterResponse::default();
        egui::ScrollArea::vertical()
            .id_salt("saved_query_parameters")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .max_height(max_height)
            .show(ui, |ui| {
                for (index, (spec, input)) in parameters.iter().zip(inputs).enumerate() {
                    let previous_kind = input.kind.clone();
                    let previous_null = input.is_null;
                    ui.push_id(index, |ui| {
                        let label = ui.label(&spec.name);
                        ui.scope(|ui| {
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
                                            ui.selectable_value(
                                                &mut input.kind,
                                                (*kind).into(),
                                                type_label(ui, kind),
                                            );
                                        }
                                    })
                                    .response
                                    .labelled_by(label.id);
                                let description = tr(ui, "saved-query-value-type");
                                ui.ctx().accesskit_node_builder(kind.id, |node| {
                                    node.set_description(description);
                                });
                                input_border(ui, &kind);
                                if focus_input && input.kind.is_empty() {
                                    kind.request_focus();
                                }
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
                            let value_id = ui.make_persistent_id("parameter_value");
                            let text_limit = max_text_bytes
                                .saturating_add(previous_kind.len())
                                .saturating_sub(input.kind.len());
                            result.rejected |= crate::text_input::reject_events_with_limits(
                                ui,
                                value_id,
                                &input.text,
                                text_limit,
                                super::MAX_INPUT_BYTES,
                            );
                            let owns_default = ui.memory(|memory| memory.has_focus(value_id))
                                && !egui::Popup::is_any_open(ui.ctx());
                            let text = ui
                                .add_enabled(
                                    !input.is_null,
                                    egui::TextEdit::singleline(&mut input.text)
                                        .id(value_id)
                                        .char_limit(super::MAX_INPUT_BYTES)
                                        .desired_width(f32::INFINITY),
                                )
                                .labelled_by(label.id);
                            input_border(ui, &text);
                            result.changed |= text.changed();
                            if focus_input && !input.is_null && !input.kind.is_empty() {
                                text.request_focus();
                            }
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
                            result.submitted |= owns_default
                                && text.lost_focus()
                                && ui.input(|input| {
                                    input.modifiers == egui::Modifiers::NONE
                                        && input.key_pressed(egui::Key::Enter)
                                });
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
                            if focus_input && input.is_null && !input.kind.is_empty() {
                                null.request_focus();
                            }
                            if null.gained_focus() {
                                null.scroll_to_me(None);
                            }

                            ui.small(hint(ui, &input.kind));
                            result.changed |=
                                previous_kind != input.kind || previous_null != input.is_null;
                        });
                        ui.add_space(4.0);
                    });
                }
            });
        result
    })
    .inner
}

#[derive(Default)]
pub(super) struct ParameterResponse {
    pub submitted: bool,
    pub rejected: bool,
    pub changed: bool,
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
