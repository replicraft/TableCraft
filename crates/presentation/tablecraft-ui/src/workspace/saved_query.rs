//! Explicit saved-query inspection and execution; no SQL reconstruction in UI.
mod editor;
mod prompt;
mod sql;
mod sql_editor;

pub(super) use sql::Confirmation as SqlCloseConfirmation;
pub use sql::{
    SavedQuerySqlCloseDecision, SavedQuerySqlCloseResult, SavedQuerySqlEligibility,
    SavedQuerySqlError, SavedQuerySqlRequest,
};
pub(in crate::workspace) use sql::{SqlResolution, copy_rebind_text};

use super::objects::memory::{self, Counter};
use super::query::parameters::{
    MAX_PARAMETERS, ParameterInput, ParameterSpec, copy_inputs, copy_text, input_copy_bytes,
    inputs_admitted, inputs_within_limits, retained_inputs,
};
use super::{Action, Data, DocumentView, State};
use crate::datasheet::Selection;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedQueryTarget {
    pub document_revision: u64,
    pub object_id: u128,
    pub name: String,
}

pub struct SavedQuery {
    pub object_id: u128,
    pub name: String,
    pub system: bool,
}

struct Pending {
    request_id: u64,
    parameters: Option<Vec<ParameterInput>>,
}

#[derive(Default)]
pub(super) struct SavedQueryState {
    pub(in crate::workspace) sql: Option<sql::Draft>,
    pub(super) active: bool,
    pub(super) target: Option<SavedQueryTarget>,
    pub(super) data: Option<Data>,
    pub(super) selection: Selection,
    parameters: Option<Vec<ParameterSpec>>,
    inputs: Vec<ParameterInput>,
    pending: Option<Pending>,
    error: Option<String>,
    prompt: Option<prompt::Session>,
    restored_inputs: Option<InputPreferences>,
}

#[derive(Clone)]
pub(super) struct InputPreferences {
    pub(super) inputs: Vec<ParameterInput>,
    pub(super) declarations: Vec<(String, String)>,
}

impl SavedQueryState {
    pub(super) fn needs_inspection(&self) -> bool {
        self.parameters.is_none()
    }

    pub(super) fn input_preferences(&self) -> Option<InputPreferences> {
        if let Some(preferences) = &self.restored_inputs {
            // NO_CLONE: Capture a distinct restoration/edit-input snapshot while the original
            // live declarations, draft, tab or totals preferences remain in use;
            // parameter edit text and declaration identity are separate owners.
            return Some(preferences.clone());
        }
        Some(InputPreferences {
            // NO_CLONE: Capture a distinct restoration/edit-input snapshot while the original
            // live declarations, draft, tab or totals preferences remain in use;
            // parameter edit text and declaration identity are separate owners.
            inputs: self.inputs.clone(),
            // NO_CLONE: Capture a distinct restoration/edit-input snapshot while the original
            // live declarations, draft, tab or totals preferences remain in use;
            // parameter edit text and declaration identity are separate owners.
            declarations: self
                .parameters
                .as_ref()?
                .iter()
                .map(|parameter| (parameter.name.clone(), parameter.kind.clone()))
                .collect(),
        })
    }

    pub(super) fn restore_preferences(
        &mut self,
        target: SavedQueryTarget,
        inputs: Option<InputPreferences>,
    ) {
        *self = Self {
            active: true,
            target: Some(target),
            restored_inputs: inputs,
            ..Self::default()
        };
    }

    pub(super) fn retained_bytes(&self) -> Option<usize> {
        let mut count = Counter::new(size_of::<Self>());
        if let Some(sql) = &self.sql {
            count.add(sql.retained_bytes()?);
        }
        if let Some(target) = &self.target {
            count.string(&target.name);
        }
        if let Some(data) = &self.data {
            count.add(memory::data(data)?);
        }
        if let Some(parameters) = &self.parameters {
            count.vector(parameters);
            for parameter in parameters {
                count.string(&parameter.name);
                count.string(&parameter.kind);
                count.string(&parameter.label);
            }
        }
        for inputs in std::iter::once(&self.inputs).chain(
            self.restored_inputs
                .iter()
                .map(|preferences| &preferences.inputs),
        ) {
            count.vector(inputs);
            for input in inputs {
                count.string(&input.name);
                count.string(&input.kind);
                count.string(&input.text);
            }
        }
        if let Some(preferences) = &self.restored_inputs {
            count.vector(&preferences.declarations);
            for (name, kind) in &preferences.declarations {
                count.string(name);
                count.string(kind);
            }
        }
        if let Some(prompt) = &self.prompt {
            count.add(prompt.retained_bytes()?);
        }
        if let Some(parameters) = self
            .pending
            .as_ref()
            .and_then(|pending| pending.parameters.as_ref())
        {
            count.add(retained_inputs(parameters)?);
        }
        count.optional(&self.error);
        count.finish()
    }
}

impl State {
    fn saved_query_binding(&self, target: &SavedQueryTarget) -> bool {
        self.database.as_ref().is_some_and(|database| {
            database.document_revision == target.document_revision
                && database
                    .queries
                    .iter()
                    .any(|query| query.object_id == target.object_id && query.name == target.name)
        })
    }

    /// Inspection is explicit and inert; an object name never authorizes a run.
    pub fn can_inspect_saved_query(&self, target: &SavedQueryTarget) -> bool {
        !self.busy
            && !self.backstage_visible()
            && !self.zoom_dialog_open()
            && !self.edit_dialog_open()
            && self.contents_request_id().is_none()
            && self.saved_query.pending.is_none()
            && self.saved_query.prompt.is_none()
            && self.saved_query_binding(target)
    }

    /// Bind an accepted inspection request and enter the saved-query view.
    pub fn begin_saved_query_inspection(&mut self, target: SavedQueryTarget, request_id: u64) {
        if self.saved_query_binding(&target)
            && !self.zoom_dialog_open()
            && !self.edit_dialog_open()
            && self.contents_request_id().is_none()
            && !self.backstage_visible()
        {
            if !self.adopt_saved_inspection(&target) {
                return;
            }
            let restored_inputs = self.saved_query.restored_inputs.take();
            let mut sql = self
                .saved_query
                .sql
                .take()
                .filter(|draft| draft.target() == &target);
            if let Some(draft) = &mut sql {
                draft.interrupt();
            }
            self.select_view(DocumentView::Query);
            self.reset_saved_query();
            self.saved_query.restored_inputs = restored_inputs;
            self.saved_query.sql = sql;
            self.saved_query.active = true;
            self.saved_query.target = Some(target);
            self.saved_query.pending = Some(Pending {
                request_id,
                parameters: None,
            });
        }
    }

    fn accepts_saved_reply(&self, request_id: u64, target: &SavedQueryTarget) -> bool {
        self.saved_query_visible()
            && !self.backstage_visible()
            && !self.zoom_dialog_open()
            && !self.edit_dialog_open()
            && self.contents_request_id().is_none()
            && self.saved_query_binding(target)
            && self.saved_query.target.as_ref() == Some(target)
            && self.saved_query.pending.as_ref().is_some_and(|pending| {
                pending.request_id == request_id
                    && pending
                        .parameters
                        .as_ref()
                        .is_none_or(|inputs| inputs == &self.saved_query.inputs)
            })
    }

    pub fn set_saved_query_details(
        &mut self,
        request_id: u64,
        target: &SavedQueryTarget,
        parameters: Vec<ParameterSpec>,
    ) -> bool {
        if !self.accepts_saved_reply(request_id, target)
            || !self
                .saved_query
                .pending
                .as_ref()
                .is_some_and(|pending| pending.parameters.is_none())
        {
            return false;
        }
        if parameters.len() > MAX_PARAMETERS {
            self.saved_query.pending = None;
            self.saved_query.error = Some(self.localizer.text("saved-query-declaration-limit"));
            return false;
        }
        let mut retained = Counter::new(0);
        retained.vector(&parameters);
        retained.count::<ParameterInput>(parameters.len());
        for parameter in &parameters {
            retained.string(&parameter.name);
            retained.string(&parameter.kind);
            retained.string(&parameter.label);
            retained.text(&parameter.name);
            retained.text(&parameter.kind);
        }
        if !self.admit_object_bytes(retained.finish()) {
            self.saved_query.pending = None;
            return false;
        }
        self.saved_query.inputs = parameters
            .iter()
            .map(|parameter| ParameterInput {
                // NO_CLONE: Capture a distinct restoration/edit-input snapshot while the
                // original live declarations, draft, tab or totals preferences remain
                // in use; parameter edit text and declaration identity are separate
                // owners.
                name: parameter.name.clone(),
                kind: if parameter.kind == "value" {
                    String::new()
                } else {
                    // NO_CLONE: Capture a distinct restoration/edit-input snapshot while the
                    // original live declarations, draft, tab or totals preferences
                    // remain in use; parameter edit text and declaration identity are
                    // separate owners.
                    parameter.kind.clone()
                },
                text: String::new(),
                is_null: false,
            })
            .collect();
        if let Some(preferences) = self.saved_query.restored_inputs.take() {
            let declarations_match = parameters.len() == preferences.declarations.len()
                && parameters.iter().zip(&preferences.declarations).all(
                    |(parameter, (name, kind))| parameter.name == *name && parameter.kind == *kind,
                );
            if declarations_match && inputs_admitted(&parameters, &preferences.inputs) {
                self.saved_query.inputs = preferences.inputs;
            } else {
                self.saved_query.error = Some(self.localizer.text("objects-parameters-changed"));
            }
        }
        self.saved_query.parameters = Some(parameters);
        self.saved_query.pending = None;
        self.bump_focus_publication();
        true
    }

    pub fn can_run_saved_query(
        &self,
        target: &SavedQueryTarget,
        parameters: &[ParameterInput],
    ) -> bool {
        !self.schema_after_awaits_reopen()
            && self.saved_parameter_copy_fits(parameters)
            && !self.saved_query.sql.as_ref().is_some_and(sql::Draft::dirty)
            && !self.busy
            && self.saved_query_visible()
            && !self.backstage_visible()
            && !self.zoom_dialog_open()
            && !self.edit_dialog_open()
            && self.contents_request_id().is_none()
            && self.saved_query_binding(target)
            && self.saved_query.target.as_ref() == Some(target)
            && self.saved_query.pending.is_none()
            && self.saved_query.prompt.is_none()
            && self.saved_query.inputs == parameters
            && self
                .saved_query
                .parameters
                .as_ref()
                .is_some_and(|specs| inputs_admitted(specs, parameters))
    }

    pub fn begin_saved_query_run(
        &mut self,
        target: &SavedQueryTarget,
        parameters: Vec<ParameterInput>,
        request_id: u64,
    ) {
        // The host already checked can_run_saved_query before dispatch. Busy
        // may now be true, so recheck the captured identity and inputs only.
        if self.saved_query_binding(target)
            && self.saved_query_visible()
            && self.saved_query.target.as_ref() == Some(target)
            && self.saved_query.inputs == parameters
            && self.saved_query.pending.is_none()
        {
            self.saved_query.pending = Some(Pending {
                request_id,
                parameters: Some(parameters),
            });
            self.saved_query.error = None;
        }
    }

    pub fn set_saved_query_data(
        &mut self,
        request_id: u64,
        target: &SavedQueryTarget,
        mut data: Data,
    ) -> bool {
        if !self.accepts_saved_reply(request_id, target)
            || !self.saved_query.pending.as_ref().is_some_and(|pending| {
                pending.parameters.as_ref() == Some(&self.saved_query.inputs)
            })
        {
            return false;
        }
        if data.source_revision.is_some()
            || data
                .query_contents
                .as_ref()
                .is_some_and(|bindings| !bindings.valid_for(&data))
            || data.row_ordinals.is_some()
            || data.offset != 0
            || data.has_more
        {
            self.saved_query.pending = None;
            self.saved_query.error = Some(self.localizer.text("saved-query-incomplete-reply"));
            return false;
        }
        if !self.admit_object_data(&data) {
            self.saved_query.pending = None;
            return false;
        }
        self.prepare_display(&mut data);
        if !self.admit_object_data(&data) {
            self.saved_query.pending = None;
            return false;
        }
        self.saved_query.selection.reset(&data);
        self.saved_query.data = Some(data);
        self.saved_query.pending = None;
        self.saved_query.error = None;
        self.bump_focus_publication();
        true
    }

    pub fn fail_saved_query(
        &mut self,
        request_id: u64,
        target: &SavedQueryTarget,
        message: String,
    ) {
        if self.accepts_saved_reply(request_id, target) {
            self.saved_query.pending = None;
            self.saved_query.error = Some(message);
        }
    }

    pub(super) fn reset_saved_query(&mut self) {
        // Preserve only the old grid's exact focus-surrender bookkeeping;
        // metadata, parameters and result rows belong to the new publication.
        let mut selection = std::mem::take(&mut self.saved_query.selection);
        selection.clear();
        self.saved_query = SavedQueryState {
            selection,
            ..Default::default()
        };
    }

    pub(super) fn interrupt_saved_query(&mut self) {
        if let Some(sql) = &mut self.saved_query.sql {
            sql.interrupt();
        }
        self.saved_query.prompt = None;
        if self.saved_query.pending.take().is_some() {
            self.saved_query.error = Some(self.localizer.text("saved-query-interrupted"));
        }
    }

    pub(super) fn saved_query_run_ready(&self) -> bool {
        self.saved_query
            .target
            .as_ref()
            .is_some_and(|target| self.can_run_saved_query(target, &self.saved_query.inputs))
    }

    pub(super) fn saved_query_run_action(&self) -> Option<Action> {
        if !self.saved_query_run_ready() {
            return None;
        }
        let target = self.saved_query.target.as_ref()?;
        let copied_target = SavedQueryTarget {
            document_revision: target.document_revision,
            object_id: target.object_id,
            name: copy_text(&target.name)?,
        };
        let parameters = copy_inputs(&self.saved_query.inputs)?;
        let copied_bytes =
            retained_inputs(&parameters)?.checked_add(copied_target.name.capacity())?;
        if self.object_bytes()?.checked_add(copied_bytes)? > super::objects::MAX_RETAINED_BYTES {
            return None;
        }
        Some(Action::RunSavedQuery {
            target: copied_target,
            parameters,
        })
    }

    /// Admission for the actual owned vector before the host dispatches a run.
    #[allow(clippy::ptr_arg)] // Spare vector capacity is part of retained-memory admission.
    pub fn can_retain_saved_query_parameters(&self, parameters: &Vec<ParameterInput>) -> bool {
        self.object_bytes()
            .and_then(|bytes| bytes.checked_add(retained_inputs(parameters)?))
            .is_some_and(|bytes| bytes <= super::objects::MAX_RETAINED_BYTES)
    }

    fn saved_parameter_copy_fits(&self, parameters: &[ParameterInput]) -> bool {
        self.object_bytes()
            .and_then(|bytes| {
                bytes
                    .checked_add(input_copy_bytes(parameters)?)?
                    .checked_add(self.saved_query.target.as_ref()?.name.len())
            })
            .is_some_and(|bytes| bytes <= super::objects::MAX_RETAINED_BYTES)
    }

    pub fn selected_saved_target(&self) -> Option<&SavedQueryTarget> {
        self.saved_query.target.as_ref()
    }

    pub(super) fn saved_query_visible(&self) -> bool {
        self.view == DocumentView::Query && self.saved_query.active
    }
}

#[cfg(test)]
mod tests;
