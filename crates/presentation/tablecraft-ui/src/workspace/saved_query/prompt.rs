//! Parameter acquisition retains saved-query authority around shared controls.
use super::SavedQueryTarget;
use crate::workspace::objects::MAX_RETAINED_BYTES;
use crate::workspace::query::parameters::prompt::{Outcome, PromptBody};
use crate::workspace::query::parameters::{copy_inputs, copy_text, input_copy_bytes};
use crate::workspace::{Action, State};
use egui::Id;

pub(super) struct Session {
    target: SavedQueryTarget,
    body: PromptBody,
    publication: u64,
}

impl Session {
    pub(super) fn retained_bytes(&self) -> Option<usize> {
        self.target
            .name
            .capacity()
            .checked_add(self.body.retained_bytes()?)
    }

    fn current(&self, state: &State) -> bool {
        self.publication == state.focus_publication
            && state.saved_query_visible()
            && !state.busy
            && !state.backstage_visible()
            && !state.edit_dialog_open()
            && state.contents_request_id().is_none()
            && state.saved_query_binding(&self.target)
            && state.saved_query.target.as_ref() == Some(&self.target)
            && state.saved_query.parameters.as_ref() == Some(&self.body.declarations)
            && state.saved_query.inputs == self.body.original
            && state.saved_query.pending.is_none()
    }
}

impl State {
    pub(in crate::workspace) fn saved_query_prompt_open(&self) -> bool {
        self.saved_query.prompt.is_some()
    }

    pub(in crate::workspace) fn saved_query_command_ready(&self) -> bool {
        self.saved_query_visible()
            && !self.saved_query_sql_dirty()
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
            let action = self.saved_query_run_action();
            if action.is_none() {
                self.error = Some(self.localizer.text("objects-memory-limit"));
            }
            return action;
        }
        let target = self.saved_query.target.as_ref()?;
        let projected = PromptBody::copy_bytes(declarations, &self.saved_query.inputs)?
            .checked_add(target.name.len())?;
        if self.object_bytes()?.checked_add(projected)? > MAX_RETAINED_BYTES {
            self.error = Some(self.localizer.text("objects-memory-limit"));
            return None;
        }
        let Some(body) = PromptBody::new(declarations, &self.saved_query.inputs) else {
            self.error = Some(self.localizer.text("objects-memory-limit"));
            return None;
        };
        let Some(name) = copy_text(&target.name) else {
            self.error = Some(self.localizer.text("objects-memory-limit"));
            return None;
        };
        let session = Session {
            target: SavedQueryTarget {
                document_revision: target.document_revision,
                object_id: target.object_id,
                name,
            },
            body,
            publication: self.focus_publication,
        };
        if self
            .object_bytes()?
            .checked_add(session.retained_bytes()?)?
            > MAX_RETAINED_BYTES
        {
            return None;
        }
        self.saved_query.prompt = Some(session);
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
        let available = self
            .object_bytes()
            .and_then(|bytes| bytes.checked_add(session.retained_bytes()?))
            .and_then(|bytes| MAX_RETAINED_BYTES.checked_sub(bytes))
            .unwrap_or(0);
        let id = Id::new((
            "saved_query_prompt",
            session.target.document_revision,
            session.target.object_id,
        ));
        let outcome = session.body.show(ctx, id, available);
        if matches!(outcome, Outcome::Cancel) || !session.current(self) {
            return;
        }
        if matches!(outcome, Outcome::Confirmed) {
            if let Some(action) = self.confirm_saved_prompt(&mut session) {
                actions.push(action);
                return;
            }
            session.body.input_rejected = true;
        }
        self.saved_query.prompt = Some(session);
    }

    fn confirm_saved_prompt(&mut self, session: &mut Session) -> Option<Action> {
        let copies =
            input_copy_bytes(&session.body.draft)?.checked_add(session.target.name.len())?;
        let base = self
            .object_bytes()?
            .checked_add(session.retained_bytes()?)?;
        if base.checked_add(copies)? > MAX_RETAINED_BYTES {
            return None;
        }
        let parameters = copy_inputs(&session.body.draft)?;
        let target = SavedQueryTarget {
            document_revision: session.target.document_revision,
            object_id: session.target.object_id,
            name: copy_text(&session.target.name)?,
        };
        let actual = crate::workspace::query::parameters::retained_inputs(&parameters)?
            .checked_add(target.name.capacity())?;
        if base.checked_add(actual)? > MAX_RETAINED_BYTES {
            return None;
        }
        if self.saved_query.inputs != session.body.draft {
            self.saved_query.inputs = std::mem::take(&mut session.body.draft);
            self.saved_query.data = None;
            self.saved_query.selection.clear();
            self.saved_query.error = None;
            self.bump_focus_publication();
        }
        Some(Action::RunSavedQuery { target, parameters })
    }
}

#[cfg(test)]
mod tests;
