//! Typed literal controls shared by saved and scratch query prompts.
use super::{ParameterInput, ParameterSpec};
use crate::localization::tr;
use crate::text_input::MAX_EDITOR_SCALARS;
use crate::theme::input_border;
use crate::workspace::focus::{self, Pane};

pub(in crate::workspace) const INPUT_KINDS: &[(&str, &str)] = &[
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

pub(in crate::workspace) fn parameters(
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
                    // NO_CLONE: Keep an independent initial/checkpoint/edit buffer before the
                    // widget or conversion mutates input; it is used for rejection,
                    // reset, comparison or preserving the original value.
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
                                MAX_EDITOR_SCALARS,
                            );
                            let owns_default = ui.memory(|memory| memory.has_focus(value_id))
                                && !egui::Popup::is_any_open(ui.ctx());
                            let mut bounded = BoundedText {
                                text: &mut input.text,
                                rejected: false,
                            };
                            let text = ui
                                .add_enabled(
                                    !input.is_null,
                                    egui::TextEdit::singleline(&mut bounded)
                                        .id(value_id)
                                        .char_limit(MAX_EDITOR_SCALARS)
                                        .desired_width(f32::INFINITY),
                                )
                                .labelled_by(label.id);
                            result.rejected |= bounded.rejected;
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
pub(in crate::workspace) struct ParameterResponse {
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

/// Reserves an independently owned edit candidate before widgets can grow it.
pub(in crate::workspace) fn candidate(
    ui: &egui::Ui,
    input: &ParameterInput,
    text_limit: usize,
    available_bytes: usize,
) -> Option<ParameterInput> {
    let incoming = ui.input(|state| {
        state.events.iter().try_fold(0usize, |bytes, event| {
            let length = match event {
                egui::Event::Text(text)
                | egui::Event::Paste(text)
                | egui::Event::Ime(egui::ImeEvent::Commit(text)) => text.len(),
                egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => text.len(),
                _ => 0,
            };
            bytes.checked_add(length)
        })
    })?;
    // Match TextEdit's undo/redo chords. The retained history is private to
    // egui, so reserve the admitted logical ceiling before restoring it.
    let restores_history = ui.input(|state| {
        state.events.iter().any(|event| {
            let egui::Event::Key {
                key,
                pressed: true,
                modifiers,
                ..
            } = event
            else {
                return false;
            };
            (*key == egui::Key::Y && modifiers.matches_logically(egui::Modifiers::COMMAND))
                || (*key == egui::Key::Z
                    && (modifiers.matches_logically(egui::Modifiers::COMMAND)
                        || modifiers
                            .matches_logically(egui::Modifiers::SHIFT | egui::Modifiers::COMMAND)))
        })
    });
    let text_bytes = if restores_history {
        text_limit.max(input.text.len())
    } else {
        input
            .text
            .len()
            .checked_add(incoming)?
            .min(text_limit)
            .max(input.text.len())
    };
    let kind_scratch = INPUT_KINDS
        .iter()
        .map(|(kind, _)| kind.len())
        .max()?
        .checked_mul(2)?;
    let required = input
        .name
        .len()
        .checked_add(input.kind.len())?
        .checked_add(text_bytes)?
        .checked_add(kind_scratch)?;
    if required > available_bytes {
        return None;
    }

    let mut text = String::new();
    text.try_reserve_exact(text_bytes).ok()?;
    text.push_str(&input.text);
    let candidate = ParameterInput {
        name: super::copy_text(&input.name)?,
        kind: super::copy_text(&input.kind)?,
        text,
        is_null: input.is_null,
    };
    let actual = candidate
        .name
        .capacity()
        .checked_add(candidate.kind.capacity())?
        .checked_add(candidate.text.capacity())?
        .checked_add(kind_scratch)?;
    (actual <= available_bytes).then_some(candidate)
}

/// TextEdit undo and insertion may not allocate beyond the admitted candidate.
struct BoundedText<'a> {
    text: &'a mut String,
    rejected: bool,
}

impl egui::TextBuffer for BoundedText<'_> {
    fn type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<BoundedText<'static>>()
    }

    fn is_mutable(&self) -> bool {
        true
    }

    fn as_str(&self) -> &str {
        self.text
    }

    fn insert_text(&mut self, text: &str, char_index: egui::text::CharIndex) -> usize {
        if text.len() > self.text.capacity().saturating_sub(self.text.len()) {
            self.rejected = true;
            return 0;
        }
        let byte_index = self
            .text
            .char_indices()
            .nth(char_index.0)
            .map_or(self.text.len(), |(index, _)| index);
        self.text.insert_str(byte_index, text);
        text.chars().count()
    }

    fn delete_char_range(&mut self, range: std::ops::Range<egui::text::CharIndex>) {
        let start = self
            .text
            .char_indices()
            .nth(range.start.0)
            .map_or(self.text.len(), |(index, _)| index);
        let end = self
            .text
            .char_indices()
            .nth(range.end.0)
            .map_or(self.text.len(), |(index, _)| index);
        self.text.replace_range(start..end, "");
    }

    fn replace_with(&mut self, text: &str) {
        if text.len() > self.text.capacity() {
            self.rejected = true;
            return;
        }
        self.text.clear();
        self.text.push_str(text);
    }
}
