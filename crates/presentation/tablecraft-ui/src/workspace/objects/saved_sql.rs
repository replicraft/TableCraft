//! Dirty saved SQL belongs to its active or parked object, never a view slot.
use super::{ObjectKind, ObjectTab, session::Session};
use crate::workspace::{
    State,
    saved_query::{SavedQueryState, SavedQueryTarget},
};

fn matches(tab: &ObjectTab, target: &SavedQueryTarget) -> bool {
    tab.target.document_revision == target.document_revision
        && matches!(&tab.target.kind, ObjectKind::SavedQuery {object_id, name}
            if *object_id == target.object_id && *name == target.name)
}

fn revision(tab: &ObjectTab, state: &SavedQueryState) -> Option<u64> {
    let draft = state.sql.as_ref()?;
    (draft.dirty() && matches(tab, draft.target())).then(|| draft.revision())
}

impl State {
    /// Includes parked objects; document replacement must respect all drafts.
    pub fn has_modified_saved_query_sql(&self) -> bool {
        self.saved_query
            .sql
            .as_ref()
            .is_some_and(|draft| draft.dirty())
            || self.objects.entries.iter().any(|entry| {
                matches!(&entry.session, Some(Session::Saved { state, .. })
                    if state.sql.as_ref().is_some_and(|draft| draft.dirty()))
            })
    }

    /// Exact dirty text revision for close/discard confirmation binding.
    pub fn saved_query_sql_revision_for(&self, tab: &ObjectTab) -> Option<u64> {
        if self.objects.active == Some(tab.id) {
            return revision(tab, &self.saved_query);
        }
        self.objects
            .entries
            .iter()
            .find(|entry| entry.tab == *tab)
            .and_then(|entry| match &entry.session {
                Some(Session::Saved { state, .. }) => revision(tab, state),
                _ => None,
            })
    }

    /// Explicit confirmed discard, bound to the exact object and text revision.
    /// A stale confirmation cannot clear a newer draft. The next inspection must
    /// reacquire its original definition; discard does not manufacture source text.
    pub fn discard_saved_query_sql(&mut self, tab: &ObjectTab, draft_revision: u64) -> bool {
        if !self.object_switch_ready()
            || self.saved_query_sql_revision_for(tab) != Some(draft_revision)
        {
            return false;
        }
        if self.objects.active == Some(tab.id) {
            self.saved_query.sql = None;
            return true;
        }
        let Some(entry) = self
            .objects
            .entries
            .iter_mut()
            .find(|entry| entry.tab == *tab)
        else {
            return false;
        };
        let Some(Session::Saved { state, .. }) = &mut entry.session else {
            return false;
        };
        state.sql = None;
        entry.retained_bytes = entry.session.as_ref().and_then(Session::retained_bytes);
        true
    }
}
