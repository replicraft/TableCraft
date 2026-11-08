//! Scratch parameter acquisition retains UI provenance, never runtime authority.
use super::prompt::{Outcome, PromptBody};
use super::{
    MAX_PARAMETERS, ParameterInput, ParameterSpec, copy_inputs, copy_text, input_copy_bytes,
    inputs_admitted, retained_inputs,
};
use crate::workspace::objects::MAX_RETAINED_BYTES;
use crate::workspace::objects::memory::Counter;
use crate::workspace::{Action, DocumentView, ObjectKind, ObjectTab, State};

pub(in crate::workspace) struct Session {
    inspection_id: u64,
    sql: String,
    sql_revision: u64,
    object: ObjectTab,
    publication: u64,
    phase: Phase,
}

enum Phase {
    Inspecting,
    Prompt(PromptBody),
    Ready(PromptBody),
}

impl Session {
    fn current(&self, state: &State) -> bool {
        state.focus_publication == self.publication
            && state.active_object().as_ref() == Some(&self.object)
            && state.view == DocumentView::Query
            && !state.saved_query.active
            && !state.backstage_visible()
            && !state.query_design_visible()
            && !state.query_design_results_visible()
            && state.query_sql_witness_current(self.sql_revision, &self.sql)
    }

    pub(in crate::workspace) fn retained_bytes(&self) -> Option<usize> {
        self.sql.capacity().checked_add(match &self.phase {
            Phase::Inspecting => 0,
            Phase::Prompt(body) | Phase::Ready(body) => body.retained_bytes()?,
        })
    }
}

impl State {
    pub(in crate::workspace) fn query_parameters_inspecting(&self) -> bool {
        self.query_parameters
            .as_ref()
            .is_some_and(|session| matches!(session.phase, Phase::Inspecting))
    }

    pub(in crate::workspace) fn scratch_parameter_prompt_open(&self) -> bool {
        self.query_parameters
            .as_ref()
            .is_some_and(|session| matches!(session.phase, Phase::Prompt(_) | Phase::Ready(_)))
    }

    pub(in crate::workspace) fn parameter_prompt_open(&self) -> bool {
        self.saved_query_prompt_open() || self.scratch_parameter_prompt_open()
    }

    pub(in crate::workspace) fn take_query_update_origin(
        &mut self,
        inspection_id: u64,
        sql: &str,
        inputs: &Vec<ParameterInput>,
    ) -> Option<crate::workspace::query::actions::update::Origin> {
        if !self.can_run_parameterized_query(inspection_id, sql, inputs) {
            return None;
        }
        let session = self.query_parameters.take()?;
        let Phase::Ready(body) = session.phase else {
            unreachable!("checked confirmed scratch parameters")
        };
        Some(crate::workspace::query::actions::update::Origin {
            sql: session.sql,
            sql_revision: session.sql_revision,
            object: session.object,
            publication: session.publication,
            parameters: body.draft,
        })
    }

    pub fn clear_query_parameter_acquisition(&mut self) {
        self.query_parameters = None;
    }

    pub fn query_parameter_inspection_current(&self, id: u64, sql: &str) -> bool {
        self.query_parameters.as_ref().is_some_and(|session| {
            session.inspection_id == id && session.sql == sql && session.current(self)
        })
    }

    /// Admit a fresh Run intent before a host captures or clears any state.
    pub fn can_inspect_query_parameters(&self) -> bool {
        self.query_ready()
            && !self.query_update_dialog_open()
            && self.query_sql_revision().is_some()
            && self.active_object().is_some_and(|tab| {
                tab.target.kind == ObjectKind::Sql
                    && self.database.as_ref().is_some_and(|database| {
                        tab.target.document_revision == database.document_revision
                    })
            })
            && !self.parameter_prompt_open()
            && !self.zoom_dialog_open()
            && !self.edit_dialog_open()
            && !self.insertion_dialog_open()
            && !self.deletion_dialog_open()
            && !self.inline_insert_active()
            && !self.insert_continuation_pending()
            && self.contents_request_id().is_none()
            && !self.find_window_open()
            && !self.recovery_prompt_open()
            && !self.query_design_dialog_open()
            && self.table_draft_save.is_none()
            && self.table_draft_close.is_none()
            && !self.table_draft_field_delete_open()
    }

    pub fn begin_query_parameter_inspection(
        &mut self,
        request_id: u64,
        captured_sql: String,
    ) -> bool {
        if request_id == 0
            || !self.can_inspect_query_parameters()
            || self.sql != captured_sql
            || !self.admit_object_bytes(Some(captured_sql.capacity()))
        {
            return false;
        }
        let Some(object) = self
            .active_object()
            .filter(|tab| tab.target.kind == ObjectKind::Sql)
        else {
            return false;
        };
        let Some(sql_revision) = self.query_sql_revision() else {
            return false;
        };
        self.query_parameters = Some(Session {
            inspection_id: request_id,
            sql: captured_sql,
            sql_revision,
            object,
            publication: self.focus_publication,
            phase: Phase::Inspecting,
        });
        true
    }

    pub fn fail_query_parameter_inspection(&mut self, request_id: u64) -> bool {
        if self.query_parameters.as_ref().is_some_and(|session| {
            session.inspection_id == request_id && matches!(session.phase, Phase::Inspecting)
        }) {
            self.query_parameters = None;
            return true;
        }
        false
    }

    pub fn set_query_parameter_details(
        &mut self,
        request_id: u64,
        captured_sql: &str,
        specs: Vec<ParameterSpec>,
    ) -> Option<Action> {
        let mut session = self.query_parameters.take()?;
        if session.inspection_id != request_id {
            self.query_parameters = Some(session);
            return None;
        }
        if session.sql != captured_sql || !session.current(self) {
            return None;
        }
        if !matches!(session.phase, Phase::Inspecting) {
            self.query_parameters = Some(session);
            return None;
        }
        let valid_specs = specs.len() <= MAX_PARAMETERS
            && specs.iter().enumerate().all(|(index, spec)| {
                !spec.name.is_empty()
                    && (spec.kind == "value"
                        || super::editor::INPUT_KINDS
                            .iter()
                            .any(|(kind, _)| *kind == spec.kind))
                    && !specs[..index]
                        .iter()
                        .any(|previous| previous.name.eq_ignore_ascii_case(&spec.name))
            });
        if !valid_specs {
            self.error = Some(self.localizer.text("query-parameters-invalid"));
            return None;
        }
        let Some(body) = self.scratch_parameter_body(&session, &specs) else {
            self.error = Some(self.localizer.text("objects-memory-limit"));
            return None;
        };
        if specs.is_empty() {
            session.phase = Phase::Ready(body);
            self.query_parameters = Some(session);
            return Some(Action::RunParameterizedQuery {
                inspection_id: request_id,
                parameters: Vec::new(),
            });
        }
        session.phase = Phase::Prompt(body);
        self.query_parameters = Some(session);
        None
    }

    #[allow(clippy::ptr_arg)] // Incoming declaration vector capacity is part of admission.
    fn scratch_parameter_body(
        &self,
        session: &Session,
        specs: &Vec<ParameterSpec>,
    ) -> Option<PromptBody> {
        if specs.len() > MAX_PARAMETERS {
            return None;
        }
        let mut incoming = Counter::new(session.retained_bytes()?);
        incoming.vector(specs);
        incoming.count::<ParameterInput>(specs.len());
        for spec in specs {
            incoming.string(&spec.name);
            incoming.string(&spec.kind);
            incoming.string(&spec.label);
            incoming.text(&spec.name);
            incoming.text(&spec.kind);
        }
        let base = self.object_bytes()?.checked_add(incoming.finish()?)?;
        if base > MAX_RETAINED_BYTES {
            return None;
        }
        let mut inputs = Vec::new();
        inputs.try_reserve_exact(specs.len()).ok()?;
        for spec in specs {
            inputs.push(ParameterInput {
                name: copy_text(&spec.name)?,
                kind: if spec.kind == "value" {
                    String::new()
                } else {
                    copy_text(&spec.kind)?
                },
                text: String::new(),
                is_null: false,
            });
        }
        let actual_inputs = retained_inputs(&inputs)?;
        let predicted_inputs = input_copy_bytes(&inputs)?;
        let base = base
            .checked_sub(predicted_inputs)?
            .checked_add(actual_inputs)?;
        if base.checked_add(PromptBody::copy_bytes(specs, &inputs)?)? > MAX_RETAINED_BYTES {
            return None;
        }
        let body = PromptBody::new(specs, &inputs)?;
        if base.checked_add(body.retained_bytes()?)? > MAX_RETAINED_BYTES {
            return None;
        }
        Some(body)
    }

    #[allow(clippy::ptr_arg)] // Actual owned vector capacity is part of admission.
    pub fn can_run_parameterized_query(
        &self,
        inspection_id: u64,
        sql: &str,
        inputs: &Vec<ParameterInput>,
    ) -> bool {
        let Some(session) = &self.query_parameters else {
            return false;
        };
        let Phase::Ready(body) = &session.phase else {
            return false;
        };
        !self.busy
            && session.inspection_id == inspection_id
            && session.sql == sql
            && session.current(self)
            && body.draft == *inputs
            && inputs_admitted(&body.declarations, inputs)
            && self
                .object_bytes()
                .and_then(|bytes| bytes.checked_add(retained_inputs(inputs)?))
                .is_some_and(|bytes| bytes <= MAX_RETAINED_BYTES)
    }

    pub fn begin_parameterized_query_run(
        &mut self,
        inspection_id: u64,
        new_request_id: u64,
        captured_sql: String,
        inputs: Vec<ParameterInput>,
    ) -> bool {
        if !self.can_run_parameterized_query(inspection_id, &captured_sql, &inputs) {
            return false;
        }
        if new_request_id == 0 {
            return false;
        }
        let retained = self
            .query_parameters
            .as_ref()
            .and_then(Session::retained_bytes);
        let admitted = self
            .object_bytes()
            .and_then(|bytes| bytes.checked_sub(retained?))
            .and_then(|bytes| bytes.checked_add(captured_sql.capacity()))
            .is_some_and(|bytes| bytes <= MAX_RETAINED_BYTES);
        if !admitted {
            self.error = Some(self.localizer.text("objects-memory-limit"));
            return false;
        }
        let session = self.query_parameters.take();
        if self.begin_query_run(new_request_id, captured_sql) {
            return true;
        }
        self.query_parameters = session;
        false
    }

    pub(in crate::workspace) fn validate_query_parameter_acquisition(&mut self) {
        if self
            .query_parameters
            .as_ref()
            .is_some_and(|session| !session.current(self))
        {
            self.query_parameters = None;
        }
    }

    pub(in crate::workspace) fn scratch_parameter_dialog(
        &mut self,
        ctx: &egui::Context,
        actions: &mut Vec<Action>,
    ) {
        let Some(mut session) = self.query_parameters.take() else {
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
        let Phase::Prompt(body) = &mut session.phase else {
            if matches!(session.phase, Phase::Ready(_)) {
                let mut cancel = false;
                egui::Modal::new(egui::Id::new((
                    "scratch_query_ready",
                    session.inspection_id,
                )))
                .show(ctx, |ui| {
                    ui.label(crate::localization::tr(ui, "query-parameters-working"));
                    cancel = ui
                        .button(crate::localization::tr(ui, "query-parameters-cancel"))
                        .clicked();
                });
                cancel |= !egui::Popup::is_any_open(ctx)
                    && ctx.input_mut(|input| {
                        input.consume_key(egui::Modifiers::NONE, egui::Key::Escape)
                    });
                if cancel {
                    return;
                }
            }
            self.query_parameters = Some(session);
            return;
        };
        let outcome = body.show(
            ctx,
            egui::Id::new(("scratch_query_parameters", session.inspection_id)),
            available,
        );
        if matches!(outcome, Outcome::Cancel) || !session.current(self) {
            return;
        }
        if matches!(outcome, Outcome::Confirmed) {
            let Phase::Prompt(body) = &session.phase else {
                unreachable!()
            };
            let additional = input_copy_bytes(&body.draft);
            let admitted = self
                .object_bytes()
                .and_then(|bytes| bytes.checked_add(session.retained_bytes()?))
                .and_then(|bytes| bytes.checked_add(additional?))
                .is_some_and(|bytes| bytes <= MAX_RETAINED_BYTES);
            if admitted && let Some(parameters) = copy_inputs(&body.draft) {
                let actual_admitted = self
                    .object_bytes()
                    .and_then(|bytes| bytes.checked_add(session.retained_bytes()?))
                    .and_then(|bytes| bytes.checked_add(retained_inputs(&parameters)?))
                    .is_some_and(|bytes| bytes <= MAX_RETAINED_BYTES);
                if actual_admitted {
                    let Phase::Prompt(body) = session.phase else {
                        unreachable!()
                    };
                    session.phase = Phase::Ready(body);
                    actions.push(Action::RunParameterizedQuery {
                        inspection_id: session.inspection_id,
                        parameters,
                    });
                    self.query_parameters = Some(session);
                    return;
                }
            }
            if let Phase::Prompt(body) = &mut session.phase {
                body.input_rejected = true;
            }
        }
        self.query_parameters = Some(session);
    }
}

#[cfg(test)]
#[path = "scratch/tests.rs"]
mod tests;
