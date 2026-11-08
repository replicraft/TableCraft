//! Terminal presentation callbacks find the original active or parked owner.
use super::{ObjectKind, ObjectTarget, session::Session};
use crate::workspace::{
    State,
    saved_query::{SavedQueryState, SavedQueryTarget},
};

fn matches(object: &ObjectTarget, target: &SavedQueryTarget) -> bool {
    object.document_revision == target.document_revision
        && matches!(&object.kind, ObjectKind::SavedQuery { object_id, name }
            if *object_id == target.object_id && *name == target.name)
}

impl State {
    pub(in crate::workspace) fn saved_sql_lifecycle_state(
        &self,
        target: &SavedQueryTarget,
    ) -> Option<&SavedQueryState> {
        if self.saved_query.target.as_ref() == Some(target) {
            return Some(&self.saved_query);
        }
        self.objects.entries.iter().find_map(|entry| {
            if !matches(&entry.tab.target, target) {
                return None;
            }
            match &entry.session {
                Some(Session::Saved { state, .. }) => Some(state),
                _ => None,
            }
        })
    }

    pub(in crate::workspace) fn saved_sql_lifecycle_state_mut(
        &mut self,
        target: &SavedQueryTarget,
    ) -> Option<&mut SavedQueryState> {
        if self.saved_query.target.as_ref() == Some(target) {
            return Some(&mut self.saved_query);
        }
        self.objects.entries.iter_mut().find_map(|entry| {
            if !matches(&entry.tab.target, target) {
                return None;
            }
            match &mut entry.session {
                Some(Session::Saved { state, .. }) => Some(state),
                _ => None,
            }
        })
    }

    pub(in crate::workspace) fn refresh_saved_sql_lifecycle_memory(
        &mut self,
        target: &SavedQueryTarget,
    ) {
        if let Some(entry) = self
            .objects
            .entries
            .iter_mut()
            .find(|entry| matches(&entry.tab.target, target))
        {
            entry.retained_bytes = match &entry.session {
                Some(session) => session.retained_bytes(),
                None => Some(0),
            };
        }
    }
}
