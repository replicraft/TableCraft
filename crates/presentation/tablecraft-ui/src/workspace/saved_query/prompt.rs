//! Parameter acquisition stages literal inputs; only final confirmation may run.
use super::{
    ParameterInput, ParameterSpec, SavedQueryTarget, editor, input_admitted, inputs_admitted,
};
use crate::localization::tr;
use crate::workspace::{Action, State};
use egui::{Id, Key, Modifiers, Popup};

const PROMPT_WIDTH: f32 = 320.0;
const MAX_PROMPT_BODY_HEIGHT: f32 = 180.0;

pub(super) struct Session {
    target: SavedQueryTarget,
    declarations: Vec<ParameterSpec>,
    original: Vec<ParameterInput>,
    draft: Vec<ParameterInput>,
    publication: u64,
    index: usize,
    focus_input: bool,
}

impl Session {
    fn current(&self, state: &State) -> bool {
        self.publication == state.focus_publication
            && state.saved_query_visible()
            && !state.busy
            && !state.backstage_visible()
            && !state.edit_dialog_open()
            && state.contents_request_id().is_none()
            && state.saved_query_binding(&self.target)
            && state.saved_query.target.as_ref() == Some(&self.target)
            && state.saved_query.parameters.as_ref() == Some(&self.declarations)
            && state.saved_query.inputs == self.original
            && state.saved_query.pending.is_none()
    }
}

impl State {
    pub(in crate::workspace) fn saved_query_prompt_open(&self) -> bool {
        self.saved_query.prompt.is_some()
    }

    pub(in crate::workspace) fn saved_query_command_ready(&self) -> bool {
        self.saved_query_visible()
            && self.saved_query.prompt.is_none()
            && super::inputs_within_limits(&self.saved_query.inputs)
            && self.saved_query.target.as_ref().is_some_and(|target| {
                self.can_inspect_saved_query(target)
                    && self.saved_query.parameters.as_ref().is_some_and(|specs| {
                        specs.len() == self.saved_query.inputs.len()
                            && specs
                                .iter()
                                .zip(&self.saved_query.inputs)
                                .all(|(spec, input)| spec.name == input.name)
                    })
            })
    }

    pub(in crate::workspace) fn request_saved_query_run(&mut self) -> Option<Action> {
        if !self.saved_query_command_ready() {
            return None;
        }
        let declarations = self.saved_query.parameters.as_ref()?;
        if declarations.is_empty() {
            return self.saved_query_run_action();
        }
        self.saved_query.prompt = Some(Session {
            target: self.saved_query.target.as_ref()?.clone(),
            declarations: declarations.clone(),
            original: self.saved_query.inputs.clone(),
            draft: self.saved_query.inputs.clone(),
            publication: self.focus_publication,
            index: 0,
            focus_input: true,
        });
        None
    }

    pub(in crate::workspace) fn saved_query_prompt_dialog(
        &mut self,
        ctx: &egui::Context,
        actions: &mut Vec<Action>,
    ) {
        let Some(mut session) = self.saved_query.prompt.take() else {
            return;
        };
        if !session.current(self) {
            return;
        }
        let popup_open = Popup::is_any_open(ctx);
        let mut confirm = false;
        let mut cancel =
            !popup_open && ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape));
        let id = Id::new((
            "saved_query_prompt",
            session.target.document_revision,
            session.target.object_id,
        ));
        egui::Modal::new(id).show(ctx, |ui| {
            ui.set_width(PROMPT_WIDTH);
            ui.label(egui::RichText::new(tr(ui, "saved-query-prompt-title")).strong());
            ui.separator();
            confirm |= ui
                .push_id(session.index, |ui| {
                    editor::parameters(
                        ui,
                        std::slice::from_ref(&session.declarations[session.index]),
                        &mut session.draft[session.index..=session.index],
                        true,
                        MAX_PROMPT_BODY_HEIGHT,
                        session.focus_input,
                    )
                })
                .inner;
            session.focus_input = false;
            if !super::inputs_within_limits(&session.draft) {
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
                let admitted = input_admitted(
                    &session.declarations[session.index],
                    &session.draft[session.index],
                ) && super::inputs_within_limits(&session.draft);
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
        if cancel || !session.current(self) {
            return;
        }
        if confirm {
            if session.index + 1 < session.declarations.len() {
                session.index += 1;
                session.focus_input = true;
            } else if inputs_admitted(&session.declarations, &session.draft) {
                if self.saved_query.inputs != session.draft {
                    self.saved_query.inputs = session.draft;
                    self.saved_query.data = None;
                    self.saved_query.selection.clear();
                    self.saved_query.error = None;
                    self.bump_focus_publication();
                }
                if let Some(action) = self.saved_query_run_action() {
                    actions.push(action);
                }
                return;
            }
        }
        self.saved_query.prompt = Some(session);
    }
}

#[cfg(test)]
mod tests;
