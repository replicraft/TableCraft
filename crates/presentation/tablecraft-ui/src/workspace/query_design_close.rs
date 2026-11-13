//! Exact Design close decisions; native save and source witness remain host-owned.
mod dialog;
#[cfg(test)]
mod tests;

use super::{Action, ObjectTab, QueryCreationSaveIdentity, SavedQuerySqlRequest, State};
use crate::workspace::objects::memory::Counter;
use std::sync::{Arc, atomic::AtomicUsize};

#[derive(Debug, PartialEq, Eq)]
pub struct QueryDesignCloseRequest {
    pub tab: ObjectTab,
    pub text_revision: u64,
    pub draft_revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryDesignCloseDecision {
    Save,
    Discard,
    Cancel,
}

#[derive(Debug)]
pub struct QueryDesignCloseResult {
    pub request: Arc<QueryDesignCloseRequest>,
    pub decision: QueryDesignCloseDecision,
}

#[derive(Default)]
pub(super) struct Model {
    intent: Option<Intent>,
    event: Option<QueryDesignCloseResult>,
}

struct Intent {
    request: Arc<QueryDesignCloseRequest>,
    text_revision: u64,
    draft_revision: u64,
    phase: Phase,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Choosing,
    SaveReady,
    SavedReady,
    Building,
    Naming,
    Creation {
        request_id: u64,
        review_id: Option<u64>,
    },
    Saved {
        request_id: u64,
        review_id: Option<u64>,
    },
}

impl Model {
    pub(in crate::workspace) fn retained_bytes(&self) -> Option<usize> {
        let mut count = Counter::new(size_of::<Self>());
        if let Some(intent) = &self.intent {
            count_request(&mut count, &intent.request);
        }
        if let Some(event) = &self.event
            && self
                .intent
                .as_ref()
                .is_none_or(|intent| !Arc::ptr_eq(&intent.request, &event.request))
        {
            count_request(&mut count, &event.request);
        }
        count.finish()
    }
}

fn count_request(count: &mut Counter, request: &QueryDesignCloseRequest) {
    count.add(size_of::<QueryDesignCloseRequest>());
    count.add(size_of::<(AtomicUsize, AtomicUsize)>() + align_of::<QueryDesignCloseRequest>());
    if let super::ObjectKind::SavedQuery { name, .. } = &request.tab.target.kind {
        count.string(name);
    }
}

impl State {
    pub fn query_design_close_open(&self) -> bool {
        self.query_design_close
            .intent
            .as_ref()
            .is_some_and(|intent| intent.phase == Phase::Choosing)
    }

    pub fn request_query_design_close(&mut self, tab: &ObjectTab) -> bool {
        if self.query_design_close.intent.is_some()
            || self.query_design_close.event.is_some()
            || self.query_creation_name_open()
            || self.query_creation_save_open()
            || !self.object_switch_ready()
        {
            return false;
        }
        if self.unresolved_query_design_edits_for(tab) {
            self.reveal_unresolved_query_design_edits_for(tab);
            return false;
        }
        let Some((text_revision, draft_revision)) = self.modified_query_design_for(tab) else {
            return false;
        };
        if !self.activate_object(tab) {
            return false;
        }
        let Some(current) = self.active_object() else {
            return false;
        };
        let name_bytes = match &current.target.kind {
            super::ObjectKind::SavedQuery { name, .. } => name.capacity(),
            _ => 0,
        };
        if !self.admit_object_bytes(name_bytes.checked_add(
            size_of::<QueryDesignCloseRequest>()
                + size_of::<(AtomicUsize, AtomicUsize)>()
                + align_of::<QueryDesignCloseRequest>(),
        )) {
            return false;
        }
        self.query_design_close.intent = Some(Intent {
            request: Arc::new(QueryDesignCloseRequest {
                tab: current,
                text_revision,
                draft_revision,
            }),
            text_revision,
            draft_revision,
            phase: Phase::Choosing,
        });
        true
    }

    pub fn choose_query_design_close(
        &mut self,
        decision: QueryDesignCloseDecision,
    ) -> Option<QueryDesignCloseResult> {
        let intent = self.query_design_close.intent.as_ref()?;
        if intent.phase != Phase::Choosing {
            return None;
        }
        if decision != QueryDesignCloseDecision::Cancel
            && self.modified_query_design_for(&intent.request.tab)
                != Some((intent.text_revision, intent.draft_revision))
        {
            return None;
        }
        if decision == QueryDesignCloseDecision::Save {
            let intent = self.query_design_close.intent.as_mut()?;
            intent.phase = Phase::SaveReady;
            // NO_CLONE: The host's explicit decision and UI continuation share
            // the immutable exact close owner until terminal success or cancellation.
            return Some(QueryDesignCloseResult {
                request: Arc::clone(&intent.request),
                decision,
            });
        }
        let intent = self.query_design_close.intent.take()?;
        if decision == QueryDesignCloseDecision::Discard
            && !self.discard_query_design_close(&intent.request)
        {
            self.query_design_close.intent = Some(intent);
            return None;
        }
        Some(QueryDesignCloseResult {
            request: intent.request,
            decision,
        })
    }

    pub fn begin_query_design_close_save(
        &mut self,
        request: &QueryDesignCloseRequest,
    ) -> Option<Action> {
        let intent = self.query_design_close.intent.as_ref()?;
        if intent.phase != Phase::SaveReady
            || intent.request.as_ref() != request
            || self.modified_query_design_for(&request.tab)
                != Some((intent.text_revision, intent.draft_revision))
            || self.active_object().as_ref() != Some(&request.tab)
        {
            self.cancel_query_design_close();
            return None;
        }
        let existing = matches!(
            request.tab.target.kind,
            super::ObjectKind::SavedQuery { .. }
        );
        let Some(action) = self.query_design_save_action_for(existing) else {
            self.cancel_query_design_close();
            return None;
        };
        self.query_design_close.intent.as_mut()?.phase = Phase::Building;
        Some(action)
    }

    pub(in crate::workspace) fn query_design_close_built(
        &mut self,
        tab_id: u64,
        draft_revision: u64,
    ) {
        let Some(intent) = &self.query_design_close.intent else {
            return;
        };
        if intent.phase != Phase::Building
            || intent.request.tab.id != tab_id
            || intent.draft_revision != draft_revision
        {
            return;
        }
        let Some((text, draft)) = self.query_design_close_stamp(&intent.request.tab) else {
            self.cancel_query_design_close();
            return;
        };
        let intent = self
            .query_design_close
            .intent
            .as_mut()
            .expect("checked close continuation");
        intent.text_revision = text;
        intent.draft_revision = draft;
        intent.phase = if matches!(intent.request.tab.target.kind, super::ObjectKind::Sql) {
            Phase::Naming
        } else {
            Phase::SavedReady
        };
    }

    pub(in crate::workspace) fn bind_query_creation_close(
        &mut self,
        request_id: u64,
        request: &super::QueryCreationRequest,
    ) {
        let Some(intent) = &mut self.query_design_close.intent else {
            return;
        };
        if intent.phase == Phase::Naming
            && intent.request.tab == request.tab
            && intent.text_revision == request.text_revision
            && request.design_revision == Some(intent.draft_revision)
        {
            intent.phase = Phase::Creation {
                request_id,
                review_id: None,
            };
        }
    }

    pub(in crate::workspace) fn bind_saved_design_close(
        &mut self,
        request_id: u64,
        request: &SavedQuerySqlRequest,
    ) {
        let Some(intent) = &mut self.query_design_close.intent else {
            return;
        };
        if intent.phase == Phase::SavedReady
            && intent.text_revision == request.draft_revision
            && request.target.document_revision == intent.request.tab.target.document_revision
            && matches!(&intent.request.tab.target.kind, super::ObjectKind::SavedQuery { object_id, name }
                if *object_id == request.target.object_id && *name == request.target.name)
        {
            intent.phase = Phase::Saved {
                request_id,
                review_id: None,
            };
        }
    }

    pub(in crate::workspace) fn review_query_creation_close(
        &mut self,
        request_id: u64,
        review_id: u64,
    ) {
        if let Some(intent) = &mut self.query_design_close.intent
            && matches!(intent.phase, Phase::Creation { request_id: actual, review_id: None } if actual == request_id)
        {
            intent.phase = Phase::Creation {
                request_id,
                review_id: Some(review_id),
            };
        }
    }

    pub(in crate::workspace) fn review_saved_design_close(
        &mut self,
        request_id: u64,
        review_id: u64,
    ) {
        if let Some(intent) = &mut self.query_design_close.intent
            && matches!(intent.phase, Phase::Saved { request_id: actual, review_id: None } if actual == request_id)
        {
            intent.phase = Phase::Saved {
                request_id,
                review_id: Some(review_id),
            };
        }
    }

    pub fn cancel_query_design_close(&mut self) -> bool {
        let Some(intent) = self.query_design_close.intent.take() else {
            return false;
        };
        self.query_design_close.event = Some(QueryDesignCloseResult {
            request: intent.request,
            decision: QueryDesignCloseDecision::Cancel,
        });
        true
    }

    pub(in crate::workspace) fn finish_query_design_close_events(
        &mut self,
        actions: &mut Vec<Action>,
    ) {
        if let Some(result) = self.query_design_close.event.take() {
            actions.push(Action::QueryDesignClose { result });
        }
    }

    /// Only call after actual acknowledged Created/full-witness publication.
    pub fn finish_query_design_close_after_creation(
        &mut self,
        identity: QueryCreationSaveIdentity,
    ) -> Option<ObjectTab> {
        let intent = self.query_design_close.intent.as_ref()?;
        if !matches!(intent.phase, Phase::Creation { request_id, review_id: Some(review_id) }
            if request_id == identity.request_id && review_id == identity.review_id)
        {
            return None;
        }
        let request = self.query_creation_save_request(identity)?;
        if request.tab != intent.request.tab
            || request.text_revision != intent.text_revision
            || request.design_revision != Some(intent.draft_revision)
        {
            return None;
        }
        let tab = self.active_object()?;
        if tab.id != intent.request.tab.id
            || tab.target.document_revision <= intent.request.tab.target.document_revision
            || !matches!(&tab.target.kind, super::ObjectKind::SavedQuery { name, .. } if *name == request.destination)
            || self
                .database
                .as_ref()
                .is_none_or(|database| database.document_revision != tab.target.document_revision)
            || self.query_design_close_stamp(&tab)
                != Some((request.text_revision.checked_add(1)?, intent.draft_revision))
            || self.query_design_close_text_for(&tab) != Some(request.sql.as_str())
        {
            return None;
        }
        self.query_design.mark_saved();
        self.query_design_close.intent = None;
        Some(tab)
    }

    /// Only call after the existing native SQL review's full-witness publication.
    pub fn finish_query_design_close_after_saved_sql(
        &mut self,
        request_id: u64,
        request: &SavedQuerySqlRequest,
        review_id: u64,
    ) -> Option<ObjectTab> {
        let intent = self.query_design_close.intent.as_ref()?;
        if !matches!(intent.phase, Phase::Saved { request_id: actual, review_id: Some(review) }
            if actual == request_id && review == review_id)
            || intent.text_revision != request.draft_revision
        {
            return None;
        }
        if !matches!(&intent.request.tab.target.kind, super::ObjectKind::SavedQuery { object_id, name }
            if *object_id == request.target.object_id && *name == request.target.name)
            || intent.request.tab.target.document_revision != request.target.document_revision
        {
            return None;
        }
        let id = intent.request.tab.id;
        let tab = self.query_design_close_tab(id)?;
        if tab.target.document_revision <= request.target.document_revision
            || !matches!(&tab.target.kind, super::ObjectKind::SavedQuery { object_id, name }
                if *object_id == request.target.object_id && *name == request.target.name)
            || self
                .database
                .as_ref()
                .is_none_or(|database| database.document_revision != tab.target.document_revision)
            || self.query_design_close_stamp(&tab)
                != Some((
                    request.draft_revision.checked_add(1)?,
                    intent.draft_revision,
                ))
            || self.query_design_close_text_for(&tab) != Some(request.proposed_sql.as_str())
            || !self.mark_query_design_saved_for(&tab)
        {
            return None;
        }
        self.query_design_close.intent = None;
        Some(tab)
    }
}

impl State {
    pub(in crate::workspace) fn cancel_query_design_close_name(&mut self) {
        if self
            .query_design_close
            .intent
            .as_ref()
            .is_some_and(|intent| intent.phase == Phase::Naming)
        {
            self.cancel_query_design_close();
        }
    }

    pub(in crate::workspace) fn cancel_query_design_close_build(&mut self) {
        if self
            .query_design_close
            .intent
            .as_ref()
            .is_some_and(|intent| intent.phase == Phase::Building)
        {
            self.cancel_query_design_close();
        }
    }

    pub(in crate::workspace) fn cancel_query_design_close_creation(&mut self, request_id: u64) {
        if self.query_design_close.intent.as_ref().is_some_and(|intent| matches!(intent.phase, Phase::Creation { request_id: actual, .. } if actual == request_id)) {
            self.cancel_query_design_close();
        }
    }

    pub(in crate::workspace) fn cancel_query_design_close_saved(&mut self, request_id: u64) {
        if self.query_design_close.intent.as_ref().is_some_and(|intent| matches!(intent.phase, Phase::Saved { request_id: actual, .. } if actual == request_id)) {
            self.cancel_query_design_close();
        }
    }
}
