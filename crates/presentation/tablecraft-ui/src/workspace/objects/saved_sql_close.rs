//! Access parked SQL through the owning object/session boundary.
use super::{ObjectTab, session::Session};
use crate::workspace::State;

impl State {
    pub(in crate::workspace) fn saved_sql_close_eligible(&self, tab: &ObjectTab) -> bool {
        if self.saved_query_sql_revision_for(tab).is_none() {
            return false;
        }
        if self.objects.active == Some(tab.id) {
            return self
                .saved_query
                .sql
                .as_ref()
                .is_some_and(|draft| draft.close_save_eligible());
        }
        self.objects
            .entries
            .iter()
            .find(|entry| entry.tab == *tab)
            .is_some_and(|entry| {
                matches!(&entry.session, Some(Session::Saved {state, ..})
                if state.sql.as_ref().is_some_and(|draft| draft.close_save_eligible()))
            })
    }

    pub(in crate::workspace) fn interrupt_saved_sql_for(&mut self, tab: &ObjectTab) {
        if self.objects.active == Some(tab.id) {
            if let Some(draft) = &mut self.saved_query.sql {
                draft.interrupt();
            }
            return;
        }
        if let Some(entry) = self
            .objects
            .entries
            .iter_mut()
            .find(|entry| entry.tab == *tab)
        {
            if let Some(Session::Saved { state, .. }) = &mut entry.session
                && let Some(draft) = &mut state.sql
            {
                draft.interrupt();
            }
            entry.retained_bytes = entry.session.as_ref().and_then(Session::retained_bytes);
        }
    }

    /// No bulk close bypasses a dirty SQL or table draft's explicit decision.
    pub fn close_all_objects(&mut self) -> bool {
        if !self.object_switch_ready()
            || self.has_modified_saved_query_sql()
            || self.has_modified_table_drafts()
        {
            return false;
        }
        for tab in self.object_tabs() {
            if !self.close_object_confirmed(&tab) {
                return false;
            }
        }
        true
    }
}
