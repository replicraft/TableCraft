//! Private exact-owner bridge to the existing object activation mechanics.
use super::{MAX_RETAINED_BYTES, ObjectKind, ObjectTab, session::Session};
use crate::workspace::{DocumentView, State};

impl State {
    /// Admit only an intent to transition. The host must first resolve the exact
    /// Number name; this never relaxes ordinary close/activation mutation guards.
    pub fn can_request_field_addition_transition(&self, tab: &ObjectTab) -> bool {
        let Some((owner, _, _, _)) = self.field_addition.draft() else {
            return false;
        };
        !self.field_addition_close_open()
            && self.can_request_field_addition_close(owner)
            && self.object_binding(&tab.target)
            && self.objects.entries.iter().any(|entry| entry.tab == *tab)
    }

    pub(in crate::workspace) fn field_addition_close_owner_bound(&self, tab: &ObjectTab) -> bool {
        let ObjectKind::Table { name } = &tab.target.kind else {
            return false;
        };
        self.object_binding(&tab.target)
            && self.database.as_ref().is_some_and(|database| {
                database
                    .tables
                    .iter()
                    .filter(|table| table.name == *name && table.local && !table.system)
                    .count()
                    == 1
            })
            && self.objects.entries.iter().any(|entry| {
                entry.tab == *tab
                    && if self.objects.active == Some(tab.id) {
                        entry.session.is_none()
                            && matches!(self.view, DocumentView::Table | DocumentView::Design)
                            && !self.saved_query.active
                    } else {
                        matches!(&entry.session, Some(Session::Table { .. }))
                    }
            })
    }

    pub(in crate::workspace) fn field_addition_close_owner_can_review(
        &self,
        tab: &ObjectTab,
    ) -> bool {
        if !self.field_addition_close_owner_bound(tab) {
            return false;
        }
        let ObjectKind::Table { name } = &tab.target.kind else {
            return false;
        };
        if self.objects.active == Some(tab.id) {
            return self.view == DocumentView::Table
                && self.data.as_ref().is_some_and(|data| {
                    data.source_revision == Some(tab.target.document_revision)
                        && data.table == *name
                });
        }
        self.objects.entries.iter().find(|entry| entry.tab == *tab).is_some_and(|entry| {
            matches!(&entry.session, Some(Session::Table { data: Some(data), design: false, .. })
                if data.source_revision == Some(tab.target.document_revision) && data.table == *name)
        })
    }

    /// Bypass only this exact unstarted naming owner after close admission.
    /// No Model is removed and ordinary activation guards remain unchanged.
    pub(in crate::workspace) fn activate_field_addition_close_owner(
        &mut self,
        tab: &ObjectTab,
        revision: u64,
    ) -> bool {
        if !self.field_addition_unstarted_matches(tab, revision)
            || !self.field_addition_close_owner_can_review(tab)
            || !self.object_switch_ready_without_saved_field_name()
            || self.field_addition_status.is_some()
            || self.has_unsaved_table_drafts()
            || self.has_modified_saved_query_sql()
            || self
                .object_bytes()
                .is_none_or(|bytes| bytes > MAX_RETAINED_BYTES)
        {
            return false;
        }
        if self.objects.active != Some(tab.id) {
            let Some(active) = self
                .objects
                .entries
                .iter()
                .find(|entry| Some(entry.tab.id) == self.objects.active)
            else {
                return false;
            };
            // Parking moves all native payload buffers. Admit the small fixed
            // boxed carriers which ordinary activation must create for that move.
            let parking = Session::empty(&active.tab.target.kind);
            if !self.admit_object_bytes(parking.retained_bytes()) {
                return false;
            }
        }
        self.activate_object_after_admission(tab)
    }
}
