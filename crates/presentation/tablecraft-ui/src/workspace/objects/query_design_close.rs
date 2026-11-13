//! Read or discard only the actual active/parked Design owner.
use super::session::Session;
use crate::workspace::{ObjectKind, ObjectTab, QueryDesignCloseRequest, State};

impl State {
    pub(in crate::workspace) fn query_design_close_stamp(
        &self,
        tab: &ObjectTab,
    ) -> Option<(u64, u64)> {
        if self.database.as_ref()?.document_revision != tab.target.document_revision
            || !self.objects.entries.iter().any(|entry| entry.tab == *tab)
        {
            return None;
        }
        if self.objects.active == Some(tab.id) {
            let text = match tab.target.kind {
                ObjectKind::Sql => self.query_sql_revision()?,
                ObjectKind::SavedQuery { .. } => self.saved_query.sql.as_ref()?.revision(),
                _ => return None,
            };
            return Some((text, self.query_design.revision()?));
        }
        match self
            .objects
            .entries
            .iter()
            .find(|entry| entry.tab == *tab)?
            .session
            .as_ref()?
        {
            Session::Sql { design, .. } => Some((self.query_sql_revision()?, design.revision()?)),
            Session::Saved { state, .. } => {
                Some((state.sql.as_ref()?.revision(), state.design.revision()?))
            }
            _ => None,
        }
    }

    pub(in crate::workspace) fn modified_query_design_for(
        &self,
        tab: &ObjectTab,
    ) -> Option<(u64, u64)> {
        let stamp = self.query_design_close_stamp(tab)?;
        let dirty = if self.objects.active == Some(tab.id) {
            self.query_design.is_dirty()
        } else {
            match self
                .objects
                .entries
                .iter()
                .find(|entry| entry.tab == *tab)?
                .session
                .as_ref()?
            {
                Session::Sql { design, .. } => design.is_dirty(),
                Session::Saved { state, .. } => state.design.is_dirty(),
                _ => false,
            }
        };
        dirty.then_some(stamp)
    }

    pub fn has_modified_query_design(&self) -> bool {
        self.objects
            .entries
            .iter()
            .any(|entry| self.modified_query_design_for(&entry.tab).is_some())
    }

    pub fn request_query_design_close_all(&mut self) -> bool {
        // NO_CLONE: The asynchronous confirmation retains exactly one owner
        // while activation mutates the registry from which it was selected.
        let Some(tab) = self
            .objects
            .entries
            .iter()
            .find(|entry| self.modified_query_design_for(&entry.tab).is_some())
            .map(|entry| entry.tab.clone())
        else {
            return false;
        };
        self.request_query_design_close(&tab)
    }

    pub(in crate::workspace) fn mark_query_design_saved_for(&mut self, tab: &ObjectTab) -> bool {
        if self.active_object().as_ref() == Some(tab) {
            self.query_design.mark_saved();
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
        match &mut entry.session {
            Some(Session::Sql { design, .. }) => design.mark_saved(),
            Some(Session::Saved { state, .. }) => state.design.mark_saved(),
            _ => return false,
        }
        entry.retained_bytes = entry.session.as_ref().and_then(Session::retained_bytes);
        true
    }

    pub(in crate::workspace) fn discard_query_design_close(
        &mut self,
        request: &QueryDesignCloseRequest,
    ) -> bool {
        if self.active_object().as_ref() != Some(&request.tab)
            || self.modified_query_design_for(&request.tab)
                != Some((request.text_revision, request.draft_revision))
            || self.unresolved_query_design_edits_for(&request.tab)
            || !self.object_switch_ready()
        {
            return false;
        }
        if let Some(revision) = self.saved_query_sql_revision_for(&request.tab)
            && !self.discard_saved_query_sql(&request.tab, revision)
        {
            return false;
        }
        self.query_design = Default::default();
        self.close_object_confirmed(&request.tab)
    }
}

impl State {
    pub(in crate::workspace) fn query_design_close_tab(&self, id: u64) -> Option<ObjectTab> {
        // NO_CLONE: Completion returns one independent exact target while the
        // caller subsequently closes/mutates the original object registry.
        self.objects
            .entries
            .iter()
            .find(|entry| entry.tab.id == id)
            .map(|entry| entry.tab.clone())
    }
}

impl State {
    pub(in crate::workspace) fn query_design_close_text_for(
        &self,
        tab: &ObjectTab,
    ) -> Option<&str> {
        self.query_design_close_stamp(tab)?;
        if self.objects.active == Some(tab.id) {
            return match tab.target.kind {
                ObjectKind::Sql => Some(self.sql.as_str()),
                ObjectKind::SavedQuery { .. } => Some(self.saved_query.sql.as_ref()?.text()),
                _ => None,
            };
        }
        match self
            .objects
            .entries
            .iter()
            .find(|entry| entry.tab == *tab)?
            .session
            .as_ref()?
        {
            Session::Sql { .. } => Some(self.sql.as_str()),
            Session::Saved { state, .. } => Some(state.sql.as_ref()?.text()),
            _ => None,
        }
    }
}
