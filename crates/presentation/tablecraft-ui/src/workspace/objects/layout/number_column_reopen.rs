//! Distinct atomic publication after a host-proven Number schema replacement.
mod admission;
mod capture;
mod publication;
#[cfg(test)]
mod tests;

use super::{boundary, catalog};
use crate::{
    Database,
    workspace::{
        State,
        number_column::{NumberColumnRequest, NumberColumnSaveIdentity},
        objects::{MAX_OBJECTS, MAX_RETAINED_BYTES, ObjectKind, ObjectTarget},
    },
};

fn bound(database: &Database, target: &ObjectTarget) -> bool {
    target.document_revision == database.document_revision
        && match &target.kind {
            ObjectKind::Table { name } => {
                database
                    .tables
                    .iter()
                    .filter(|table| table.name == *name)
                    .count()
                    == 1
            }
            ObjectKind::SavedQuery { object_id, name } => {
                database
                    .queries
                    .iter()
                    .filter(|query| query.object_id == *object_id && query.name == *name)
                    .count()
                    == 1
            }
            ObjectKind::Sql => true,
            ObjectKind::DraftTable { .. } => false,
        }
}

impl State {
    /// Structural preflight for this exact retained Number naming request.
    /// Global busy and this operation's own status are allowed. This is neither
    /// native source admission nor evidence that a future reopen will succeed.
    pub fn can_rebind_number_column_layout(&self, request: &NumberColumnRequest) -> bool {
        let Some(database) = &self.database else {
            return false;
        };
        let Some(active) = self.objects.active else {
            return false;
        };
        self.number_column_request_is_current(request)
            && request.tab.target.document_revision == database.document_revision
            && matches!(&request.tab.target.kind, ObjectKind::Table { name }
                if database.tables.iter().filter(|table| table.name == *name && table.local && !table.system).count() == 1)
            && self.objects.revision == Some(database.document_revision)
            && !self.objects.entries.is_empty()
            && self.objects.entries.len() <= MAX_OBJECTS
            && self
                .objects
                .entries
                .iter()
                .any(|entry| entry.tab == request.tab)
            && self
                .objects
                .entries
                .iter()
                .any(|entry| entry.tab.id == active && entry.session.is_none())
            && self
                .objects
                .entries
                .iter()
                .enumerate()
                .all(|(position, entry)| {
                    bound(database, &entry.tab.target)
                        && self.objects.entries[..position].iter().all(|old| {
                            old.tab.id != entry.tab.id && old.tab.target != entry.tab.target
                        })
                })
            && self
                .objects
                .navigation
                .as_ref()
                .is_none_or(|target| bound(database, target))
            && boundary::current(self)
            && !self.has_unsaved_table_drafts()
            && !self.has_modified_saved_query_sql()
            && self.table_draft.is_none()
            && !self.saved_query_sql_close_open()
            && !self.number_column_close_open()
            && self.saved_query_sql_save_status().is_none()
            && self.table_draft_close.is_none()
            && !self.zoom_dialog_open()
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
            && self
                .object_bytes()
                .is_some_and(|bytes| bytes <= MAX_RETAINED_BYTES)
            && admission::fits(self, database) == Some(true)
    }

    /// Publish only after the host acknowledged a known AFTER Number-column
    /// receipt AND successfully reopened its complete AddedColumnReopen witness.
    /// The witness must belong to this exact request/review/save identity and
    /// prove the same-name column addition, preserving other object definitions.
    /// These are explicit caller preconditions, never inferred from Database.
    ///
    /// This separate schema route does not accept query-only publication proof.
    /// It preserves tab identities/order, scratch SQL, clean saved SQL text and
    /// source-neutral preferences, invalidating native results/handles/pending
    /// authority. Host-owned filter/sort definitions require fresh validation.
    /// The Number model/status remain retained until the host clears them after
    /// successful publication. A refusal leaves current state/text unchanged.
    pub fn resolve_number_column_after_reopen(
        &mut self,
        request_id: u64,
        request: &NumberColumnRequest,
        review_id: u64,
        save_id: u64,
        database: Database,
    ) -> bool {
        let identity = NumberColumnSaveIdentity {
            request_id,
            review_id,
            save_id,
        };
        if !self.matches_number_column_save(request, identity)
            || !self.can_rebind_number_column_layout(request)
            || self
                .database
                .as_ref()
                .is_none_or(|old| !catalog::matches(old, &database))
            || admission::fits(self, &database) != Some(true)
        {
            return false;
        }
        let Some(prepared) = capture::prepare(self, &database) else {
            return false;
        };
        publication::publish(self, prepared, database)
    }
}
