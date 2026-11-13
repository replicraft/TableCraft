//! Read or discard only the actual active/parked Design owner.
use super::session::Session;
use crate::workspace::{ObjectKind, ObjectTab, QueryDesignCloseRequest, State};

impl State {
    pub(in crate::workspace) fn query_design_close_stamp(
        &self,
        tab: &ObjectTab,
    ) -> Option<(u64, Option<u64>)> {
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
            let (result, sql) = if self.saved_query.active {
                (
                    &self.saved_query.design_result,
                    self.saved_query.sql.as_ref()?.text(),
                )
            } else {
                (&self.query_result, self.sql.as_str())
            };
            return Some((text, self.query_design.close_revision(result, sql)));
        }
        match self
            .objects
            .entries
            .iter()
            .find(|entry| entry.tab == *tab)?
            .session
            .as_ref()?
        {
            Session::Sql {
                design,
                result_origin,
                ..
            } => Some((
                self.query_sql_revision()?,
                design.close_revision(result_origin, &self.sql),
            )),
            Session::Saved { state, .. } => Some((
                state.sql.as_ref()?.revision(),
                state
                    .design
                    .close_revision(&state.design_result, state.sql.as_ref()?.text()),
            )),
            _ => None,
        }
    }

    pub(in crate::workspace) fn modified_query_design_for(
        &self,
        tab: &ObjectTab,
    ) -> Option<(u64, Option<u64>)> {
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
        (dirty && stamp.1.is_some()).then_some(stamp)
    }

    pub(in crate::workspace) fn modified_query_close_for(
        &self,
        tab: &ObjectTab,
    ) -> Option<(u64, Option<u64>)> {
        self.modified_query_design_for(tab).or_else(|| {
            (tab.target.kind == ObjectKind::Sql && !self.sql.is_empty())
                .then(|| self.query_design_close_stamp(tab))
                .flatten()
        })
    }

    /// A scratch query has no native persisted definition. Its exact empty
    /// authoring baseline is independent of text/request freshness revisions.
    pub fn has_unsaved_scratch_query(&self) -> bool {
        self.objects.entries.iter().any(|entry| {
            entry.tab.target.kind == ObjectKind::Sql
                && self.modified_query_close_for(&entry.tab).is_some()
        })
    }

    pub fn request_scratch_query_close(&mut self) -> bool {
        let Some(tab) = self
            .objects
            .entries
            .iter()
            .find(|entry| {
                entry.tab.target.kind == ObjectKind::Sql
                    && self.modified_query_close_for(&entry.tab).is_some()
            })
            .map(|entry| ObjectTab {
                id: entry.tab.id,
                target: super::ObjectTarget {
                    document_revision: entry.tab.target.document_revision,
                    kind: ObjectKind::Sql,
                },
            })
        else {
            return false;
        };
        self.request_query_design_close(&tab)
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

    pub(in crate::workspace) fn query_design_baseline_current_for(&self, tab: &ObjectTab) -> bool {
        if self.active_object().as_ref() == Some(tab) {
            return self.query_design.saved_baseline_current();
        }
        match self
            .objects
            .entries
            .iter()
            .find(|entry| entry.tab == *tab)
            .and_then(|entry| entry.session.as_ref())
        {
            Some(Session::Sql { design, .. }) => design.saved_baseline_current(),
            Some(Session::Saved { state, .. }) => state.design.saved_baseline_current(),
            _ => false,
        }
    }

    pub(in crate::workspace) fn discard_query_design_close(
        &mut self,
        request: &QueryDesignCloseRequest,
    ) -> bool {
        if self.active_object().as_ref() != Some(&request.tab)
            || self.modified_query_close_for(&request.tab)
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
        self.close_object_after_design_check(&request.tab)
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
