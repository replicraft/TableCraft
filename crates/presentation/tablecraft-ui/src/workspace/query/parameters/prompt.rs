//! Staged prompt controls, independent of query identity or execution authority.
use super::{
    MAX_INPUT_BYTES, ParameterInput, ParameterSpec, copy_inputs, copy_text, editor, input_admitted,
    input_bytes, input_copy_bytes, inputs_admitted, inputs_within_limits, retained_inputs,
};
use crate::localization::tr;
use crate::workspace::objects::memory::Counter;
use egui::{Id, Key, Modifiers, Popup};

const PROMPT_WIDTH: f32 = 320.0;
const MAX_PROMPT_BODY_HEIGHT: f32 = 180.0;

pub(in crate::workspace) struct PromptBody {
    pub(in crate::workspace) declarations: Vec<ParameterSpec>,
    pub(in crate::workspace) original: Vec<ParameterInput>,
    pub(in crate::workspace) draft: Vec<ParameterInput>,
    pub(in crate::workspace) index: usize,
    pub(in crate::workspace) focus_input: bool,
    pub(in crate::workspace) input_rejected: bool,
}

pub(in crate::workspace) enum Outcome {
    Pending,
    Cancel,
    Confirmed,
}

impl PromptBody {
    pub(in crate::workspace) fn copy_bytes(
        specs: &[ParameterSpec],
        inputs: &[ParameterInput],
    ) -> Option<usize> {
        let mut count = Counter::new(0);
        count.count::<ParameterSpec>(specs.len());
        for spec in specs {
            count.string(&spec.name);
            count.string(&spec.kind);
            count.string(&spec.label);
        }
        count.add(input_copy_bytes(inputs)?.checked_mul(2)?);
        count.finish()
    }

    pub(in crate::workspace) fn new(
        specs: &[ParameterSpec],
        inputs: &[ParameterInput],
    ) -> Option<Self> {
        let mut declarations = Vec::new();
        declarations.try_reserve_exact(specs.len()).ok()?;
        for spec in specs {
            declarations.push(ParameterSpec {
                name: copy_text(&spec.name)?,
                kind: copy_text(&spec.kind)?,
                label: copy_text(&spec.label)?,
                nullable: spec.nullable,
            });
        }
        Some(Self {
            declarations,
            original: copy_inputs(inputs)?,
            draft: copy_inputs(inputs)?,
            index: 0,
            focus_input: true,
            input_rejected: false,
        })
    }

    pub(in crate::workspace) fn retained_bytes(&self) -> Option<usize> {
        let mut count = Counter::new(0);
        count.vector(&self.declarations);
        for spec in &self.declarations {
            count.string(&spec.name);
            count.string(&spec.kind);
            count.string(&spec.label);
        }
        count.add(retained_inputs(&self.original)?);
        count.add(retained_inputs(&self.draft)?);
        count.finish()
    }

    pub(in crate::workspace) fn show(
        &mut self,
        ctx: &egui::Context,
        id: Id,
        available_bytes: usize,
    ) -> Outcome {
        let popup_open = Popup::is_any_open(ctx);
        let mut confirm = false;
        let mut cancel =
            !popup_open && ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape));
        egui::Modal::new(id).show(ctx, |ui| {
            ui.set_width(PROMPT_WIDTH);
            ui.label(egui::RichText::new(tr(ui, "saved-query-prompt-title")).strong());
            ui.separator();
            let used_bytes = input_bytes(&self.draft).unwrap_or(usize::MAX);
            let other_bytes = used_bytes.saturating_sub(self.draft[self.index].text.len());
            let text_limit = MAX_INPUT_BYTES.saturating_sub(other_bytes);
            let response = ui
                .push_id(self.index, |ui| {
                    self.edit_current(ui, text_limit, available_bytes)
                })
                .inner;
            confirm |= response.submitted;
            if response.rejected {
                self.input_rejected = true;
            } else if response.changed {
                self.input_rejected = false;
            }
            self.focus_input = false;
            if self.input_rejected || !inputs_within_limits(&self.draft) {
                ui.small(tr(ui, "saved-query-input-limit"));
            }
            ui.small(tr(ui, "saved-query-prompt-validation"));
            ui.separator();
            let ok_label = tr(ui, "saved-query-prompt-ok");
            let cancel_label = tr(ui, "saved-query-prompt-cancel");
            let button_width = |label: &str| {
                let text = egui::WidgetText::from(label).into_galley(
                    ui,
                    Some(egui::TextWrapMode::Extend),
                    f32::INFINITY,
                    egui::TextStyle::Button,
                );
                text.size().x + ui.spacing().button_padding.x * 2.0
            };
            let ok_width = button_width(&ok_label);
            let cancel_width = button_width(&cancel_label);
            let group_width = ok_width + cancel_width + ui.spacing().item_spacing.x;
            let button_height = ui.spacing().interact_size.y;
            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - group_width).max(0.0));
                let admitted =
                    input_admitted(&self.declarations[self.index], &self.draft[self.index])
                        && !self.input_rejected
                        && inputs_within_limits(&self.draft);
                let ok = ui
                    .add_enabled_ui(admitted, |ui| {
                        ui.add_sized(
                            egui::vec2(ok_width, button_height),
                            egui::Button::new(ok_label).wrap_mode(egui::TextWrapMode::Extend),
                        )
                    })
                    .inner;
                confirm = admitted && (confirm || ok.clicked());
                cancel |= ui
                    .add_sized(
                        egui::vec2(cancel_width, button_height),
                        egui::Button::new(cancel_label).wrap_mode(egui::TextWrapMode::Extend),
                    )
                    .clicked();
            });
        });
        if cancel {
            return Outcome::Cancel;
        }
        if confirm {
            if self.index + 1 < self.declarations.len() {
                self.index += 1;
                self.focus_input = true;
            } else if inputs_admitted(&self.declarations, &self.draft) {
                return Outcome::Confirmed;
            }
        }
        Outcome::Pending
    }

    fn edit_current(
        &mut self,
        ui: &mut egui::Ui,
        text_limit: usize,
        available_bytes: usize,
    ) -> editor::ParameterResponse {
        let Some(mut candidate) =
            editor::candidate(ui, &self.draft[self.index], text_limit, available_bytes)
        else {
            let attempted_edit = ui.input(|input| {
                input.events.iter().any(|event| {
                    matches!(
                        event,
                        egui::Event::Text(_) | egui::Event::Paste(_) | egui::Event::Ime(_)
                    )
                })
            });
            ui.add_enabled_ui(false, |ui| {
                editor::parameters(
                    ui,
                    std::slice::from_ref(&self.declarations[self.index]),
                    &mut self.draft[self.index..=self.index],
                    MAX_PROMPT_BODY_HEIGHT,
                    false,
                    text_limit,
                )
            });
            return editor::ParameterResponse {
                rejected: attempted_edit,
                ..Default::default()
            };
        };
        let limit = text_limit.min(candidate.text.capacity());
        let response = editor::parameters(
            ui,
            std::slice::from_ref(&self.declarations[self.index]),
            std::slice::from_mut(&mut candidate),
            MAX_PROMPT_BODY_HEIGHT,
            self.focus_input,
            limit,
        );
        if !response.rejected && response.changed {
            self.draft[self.index] = candidate;
        }
        response
    }
}
