//! Atomic UI publication after an explicitly host-proven query-only replacement.
mod admission;
mod capture;
mod publication;

use super::{boundary, catalog};

use crate::workspace::{
    State,
    objects::{MAX_OBJECTS, MAX_RETAINED_BYTES, ObjectKind, ObjectTarget},
};
use crate::{Database, SavedQuerySqlRequest};

fn bound(database: &Database, target: &ObjectTarget) -> bool {
    if target.document_revision != database.document_revision {
        return false;
    }
    match &target.kind {
        ObjectKind::Table { name } => database.tables.iter().any(|table| table.name == *name),
        ObjectKind::SavedQuery { object_id, name } => database
            .queries
            .iter()
            .any(|query| query.object_id == *object_id && query.name == *name),
        ObjectKind::Sql => true,
        ObjectKind::DraftTable { .. } => false,
    }
}

impl State {
    /// Structural preflight before native review/save; global busy is not a gate.
    /// It grants no native authority and does not predict future reopen success.
    /// All opened table/query/scratch owners can survive the scoped publication.
    pub fn can_rebind_saved_query_sql_layout(&self, request: &SavedQuerySqlRequest) -> bool {
        let Some(database) = &self.database else {
            return false;
        };
        let Some(active) = self.objects.active else {
            return false;
        };
        let target_count = database
            .queries
            .iter()
            .filter(|query| {
                database.document_revision == request.target.document_revision
                    && query.object_id == request.target.object_id
                    && query.name == request.target.name
            })
            .count();
        target_count == 1
            && boundary::current(self)
            && self.objects.revision == Some(database.document_revision)
            && !self.objects.entries.is_empty()
            && self.objects.entries.len() <= MAX_OBJECTS
            && self
                .objects
                .entries
                .iter()
                .any(|entry| entry.tab.id == active && entry.session.is_none())
            && self
                .objects
                .entries
                .iter()
                .all(|entry| bound(database, &entry.tab.target))
            && !self.has_unsaved_table_drafts()
            && self.table_draft.is_none()
            && !self.saved_query_sql_close_open()
            && self.table_draft_close.is_none()
            && !self.edit_dialog_open()
            && !self.insertion_dialog_open()
            && !self.deletion_dialog_open()
            && !self.recovery_prompt_open()
            && self.contents_request_id().is_none()
            && !self.parameter_prompt_open()
            && !self.query_action_dialog_open()
            && !self.table_draft_save_open()
            && !self.table_draft_field_delete_open()
            && self.query_sql_revision.checked_add(1).is_some()
            && request.draft_revision.checked_add(1).is_some()
            && self
                .saved_sql_lifecycle_state(&request.target)
                .and_then(|state| state.sql.as_ref())
                .is_some_and(|draft| draft.matches_rebind_request(request))
            && self
                .object_bytes()
                .is_some_and(|bytes| bytes <= MAX_RETAINED_BYTES)
    }

    /// Publish only AFTER a known replacement acknowledgement (or a verified
    /// native unchanged review) AND successful full-witness reopen through the
    /// runtime's query-only save route.
    ///
    /// Those are explicit caller preconditions, not facts inferred from Database
    /// metadata or this presentation request. This method grants no backend
    /// authority. The host retains source-neutral native filter definitions for
    /// new-source reapplication and releases obsolete handles after success.
    ///
    /// Every active/parked SQL draft is transferred explicitly. Only this exact
    /// accepted request's baseline resolves; other original/proposed text, dirty
    /// revisions and errors survive. All old pending authority/results invalidate.
    /// A refusal leaves the current catalog, owners and text unchanged.
    pub fn resolve_saved_query_sql_review_after_reopen(
        &mut self,
        request_id: u64,
        request: &SavedQuerySqlRequest,
        review_id: u64,
        database: Database,
    ) -> bool {
        if !self.can_rebind_saved_query_sql_layout(request)
            || !self
                .saved_sql_lifecycle_state(&request.target)
                .and_then(|state| state.sql.as_ref())
                .is_some_and(|draft| draft.accepts_rebind_review(request_id, request, review_id))
            || !self
                .database
                .as_ref()
                .is_some_and(|old| catalog::matches(old, &database))
        {
            return false;
        }
        if !admission::fits(self, request, &database).is_some_and(|fits| fits) {
            return false;
        }
        let Some(prepared) = capture::prepare(self, request, &database) else {
            return false;
        };
        publication::publish(self, prepared, database)
    }
}

#[cfg(test)]
mod tests;
