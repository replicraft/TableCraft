//! Capture the exact active authoring owner after widgets, never stored-copy SQL.
use super::{
    Pending, QueryCreationRequest, SHARED_REQUEST_CONTROL_BYTES, SHARED_TEXT_CONTROL_BYTES,
};
use crate::workspace::{Action, ObjectKind, ObjectTab, State, query::parameters::copy_text};
use std::sync::Arc;

pub(super) struct Name {
    pub(super) tab: ObjectTab,
    pub(super) text_revision: u64,
    pub(super) design_revision: Option<u64>,
    pub(super) text: String,
    pub(super) focus_requested: bool,
}

impl State {
    pub fn query_creation_request_current(&self, request: &QueryCreationRequest) -> bool {
        self.active_object().as_ref() == Some(&request.tab)
            && self
                .database
                .as_ref()
                .is_some_and(|database| database.document_revision == request.document_revision)
            && self.query_creation_text_revision() == Some(request.text_revision)
            && self.query_creation_design_revision() == request.design_revision
            && self.query_creation_sql() == Some(request.sql.as_str())
            && match &request.source {
                Some(source) => self.selected_saved_target() == Some(source),
                None => request.tab.target.kind == ObjectKind::Sql,
            }
    }

    pub fn begin_query_creation_review(
        &mut self,
        request_id: u64,
        request: Arc<QueryCreationRequest>,
    ) -> bool {
        if request_id == 0
            || self.query_creation.pending.is_some()
            || self.query_creation.save.is_some()
            || !self.query_creation_request_current(&request)
            || !self.admit_object_bytes(request.retained_bytes())
        {
            return false;
        }
        self.bind_query_creation_close(request_id, &request);
        self.query_creation.pending = Some(Pending {
            request_id,
            request,
            review_id: None,
        });
        true
    }

    pub fn set_query_creation_review(
        &mut self,
        request_id: u64,
        request: &QueryCreationRequest,
        review_id: u64,
    ) -> bool {
        if review_id == 0 || !self.query_creation_request_current(request) {
            return false;
        }
        let Some(pending) = &mut self.query_creation.pending else {
            return false;
        };
        if pending.request_id != request_id
            || pending.request.as_ref() != request
            || pending.review_id.is_some()
        {
            return false;
        }
        pending.review_id = Some(review_id);
        self.review_query_creation_close(request_id, review_id);
        true
    }

    pub fn matches_query_creation_review(
        &self,
        request_id: u64,
        request: &QueryCreationRequest,
        review_id: u64,
    ) -> bool {
        self.query_creation.pending.as_ref().is_some_and(|pending| {
            pending.request_id == request_id
                && pending.request.as_ref() == request
                && pending.review_id == Some(review_id)
        })
    }

    pub fn fail_query_creation(
        &mut self,
        request_id: u64,
        request: &QueryCreationRequest,
        message: String,
    ) -> bool {
        if self.query_creation.save.is_some() {
            return false;
        }
        let matches = self.query_creation.pending.as_ref().is_some_and(|pending| {
            pending.request_id == request_id && pending.request.as_ref() == request
        });
        if !matches {
            return false;
        }
        self.cancel_query_design_close_creation(request_id);
        let admitted = self.admit_object_bytes(Some(message.capacity()));
        let restored = self
            .restore_query_creation_name(request, if admitted { message.capacity() } else { 0 });
        self.query_creation.pending = None;
        if admitted && restored != Some(false) {
            self.error = Some(message);
        }
        true
    }

    pub fn release_query_creation_review(
        &mut self,
        request_id: u64,
        request: &QueryCreationRequest,
        review_id: u64,
        message: Option<String>,
    ) -> bool {
        if self.query_creation.save.is_some()
            || !self.query_creation.pending.as_ref().is_some_and(|pending| {
                pending.request_id == request_id
                    && pending.request.as_ref() == request
                    && pending.review_id.is_none_or(|actual| actual == review_id)
            })
        {
            return false;
        }
        self.cancel_query_design_close_creation(request_id);
        let admitted = message
            .as_ref()
            .is_none_or(|message| self.admit_object_bytes(Some(message.capacity())));
        let restored = self.restore_query_creation_name(
            request,
            if admitted {
                message.as_ref().map_or(0, String::capacity)
            } else {
                0
            },
        );
        self.query_creation.pending = None;
        if admitted
            && restored != Some(false)
            && let Some(message) = message
        {
            self.error = Some(message);
        }
        true
    }

    fn restore_query_creation_name(
        &mut self,
        request: &QueryCreationRequest,
        report_bytes: usize,
    ) -> Option<bool> {
        if !self.query_creation_request_current(request) || self.query_creation.name.is_some() {
            return None;
        }
        let owner_bytes = match &request.tab.target.kind {
            ObjectKind::SavedQuery { name, .. } => name.len(),
            _ => 0,
        };
        let bytes = request
            .destination
            .len()
            .checked_add(owner_bytes)
            .and_then(|bytes| bytes.checked_add(report_bytes));
        let Some(bytes) = bytes else {
            self.admit_object_bytes(None);
            return Some(false);
        };
        if !self.admit_object_bytes(Some(bytes)) {
            return Some(false);
        }
        // NO_CLONE: Retry keeps the exact failed destination independently of the
        // immutable issued request still retained by the asynchronous host.
        let Some(text) = copy_text(&request.destination) else {
            self.error = Some(self.localizer.text("objects-memory-limit"));
            return Some(false);
        };
        let tab = self.active_object()?;
        let owner_bytes = match &tab.target.kind {
            ObjectKind::SavedQuery { name, .. } => name.capacity(),
            _ => 0,
        };
        let bytes = text
            .capacity()
            .checked_add(owner_bytes)
            .and_then(|bytes| bytes.checked_add(report_bytes));
        let Some(bytes) = bytes else {
            self.admit_object_bytes(None);
            return Some(false);
        };
        if !self.admit_object_bytes(Some(bytes)) {
            return Some(false);
        }
        self.query_creation.name = Some(Name {
            tab,
            text_revision: request.text_revision,
            design_revision: request.design_revision,
            text,
            focus_requested: false,
        });
        Some(true)
    }

    pub fn query_creation_name_open(&self) -> bool {
        self.query_creation.name.is_some()
    }

    pub fn begin_query_creation_name(&mut self) -> bool {
        if self.busy
            || self.native_mutation_blocked()
            || self.query_creation.name.is_some()
            || self.query_creation.pending.is_some()
            || self.query_creation.save.is_some()
            || self.query_design.has_unresolved_field_inputs()
        {
            return false;
        }
        let Some(tab) = self.active_object() else {
            return false;
        };
        if !matches!(
            tab.target.kind,
            ObjectKind::Sql | ObjectKind::SavedQuery { .. }
        ) {
            return false;
        }
        let Some(text_revision) = self.query_creation_text_revision() else {
            return false;
        };
        self.query_creation.name = Some(Name {
            tab,
            text_revision,
            design_revision: self.query_creation_design_revision(),
            text: String::new(),
            focus_requested: false,
        });
        true
    }

    pub fn set_query_creation_name(&mut self, text: String) -> bool {
        if !self.admit_object_bytes(Some(text.capacity())) {
            return false;
        }
        let Some(name) = &mut self.query_creation.name else {
            return false;
        };
        name.text = text;
        true
    }

    pub fn cancel_query_creation_name(&mut self) -> bool {
        let cancelled = self.query_creation.name.take().is_some();
        if cancelled {
            self.cancel_query_design_close_name();
        }
        cancelled
    }

    pub fn submit_query_creation_name(&mut self) -> Option<Action> {
        let name = self.query_creation.name.as_ref()?;
        if self.active_object().as_ref() != Some(&name.tab)
            || self.query_creation_text_revision() != Some(name.text_revision)
            || self.query_creation_design_revision() != name.design_revision
            || name.text.trim().is_empty()
        {
            return None;
        }
        let additional = self
            .query_creation_sql()?
            .len()
            .checked_add(
                self.selected_saved_target()
                    .map_or(0, |source| source.name.len()),
            )?
            .checked_add(size_of::<QueryCreationRequest>())?
            .checked_add(SHARED_TEXT_CONTROL_BYTES + SHARED_REQUEST_CONTROL_BYTES)?;
        if !self.admit_object_bytes(Some(additional)) {
            return None;
        }
        let sql = copy_text(self.query_creation_sql()?)?;
        let source = match self.selected_saved_target() {
            Some(source) => Some(crate::SavedQueryTarget {
                document_revision: source.document_revision,
                object_id: source.object_id,
                name: copy_text(&source.name)?,
            }),
            None => None,
        };
        let actual = sql
            .capacity()
            .checked_add(source.as_ref().map_or(0, |source| source.name.capacity()))?
            .checked_add(size_of::<QueryCreationRequest>())?
            .checked_add(SHARED_TEXT_CONTROL_BYTES + SHARED_REQUEST_CONTROL_BYTES)?;
        if !self.admit_object_bytes(Some(actual)) {
            return None;
        }
        let name = self.query_creation.name.take()?;
        let request = QueryCreationRequest {
            document_revision: name.tab.target.document_revision,
            tab: name.tab,
            text_revision: name.text_revision,
            design_revision: name.design_revision,
            source,
            sql: Arc::new(sql),
            destination: name.text,
        };
        Some(Action::ReviewQueryCreation {
            request: Arc::new(request),
        })
    }

    fn query_creation_sql(&self) -> Option<&str> {
        match self.active_object()?.target.kind {
            ObjectKind::Sql => Some(&self.sql),
            ObjectKind::SavedQuery { .. } => {
                self.saved_query_sql().map(|(_, proposed, _, _)| proposed)
            }
            _ => None,
        }
    }

    fn query_creation_text_revision(&self) -> Option<u64> {
        match self.active_object()?.target.kind {
            ObjectKind::Sql => self.query_sql_revision(),
            ObjectKind::SavedQuery { .. } => {
                self.saved_query_sql().map(|(_, _, revision, _)| revision)
            }
            _ => None,
        }
    }

    fn query_creation_design_revision(&self) -> Option<u64> {
        self.query_design_visible()
            .then(|| {
                self.query_design_request()
                    .map(|request| request.draft_revision)
            })
            .flatten()
    }
}

impl State {
    pub fn can_begin_query_creation_save_as(&self) -> bool {
        !self.busy
            && !self.native_mutation_blocked()
            && self.query_creation.name.is_none()
            && self.query_creation.pending.is_none()
            && self.query_creation.save.is_none()
            && !self.saved_query_sql_pending()
            && !self.query_design.has_unresolved_field_inputs()
            && self
                .query_creation_sql()
                .is_some_and(|sql| !sql.trim().is_empty() || self.query_design_visible())
            && self.database.as_ref().is_some_and(|database| {
                self.active_object()
                    .is_some_and(|tab| tab.target.document_revision == database.document_revision)
            })
    }

    pub(in crate::workspace) fn can_begin_query_creation(&self) -> bool {
        self.can_begin_query_creation_save_as()
            && self
                .active_object()
                .is_some_and(|tab| tab.target.kind == ObjectKind::Sql)
    }
}
