//! Rename-specific publication over the actual source-neutral replacement owner.
#[cfg(test)]
mod tests;

use super::{catalog, replacement};
use crate::{
    Database,
    workspace::{
        State,
        field_rename::{FieldRenameRequest, FieldRenameSaveIdentity},
        objects::{MAX_OBJECTS, ObjectKind},
    },
};

impl State {
    /// Structural UI preflight, not native source or receipt authority. Own busy
    /// and status are allowed; unrelated dirty/pending owners must remain intact.
    pub fn can_rebind_field_rename_layout(&self, request: &FieldRenameRequest) -> bool {
        let Some(database) = &self.database else {
            return false;
        };
        self.field_rename_request_is_current(request)
            && request.target.tab.target.document_revision == database.document_revision
            && match &request.target.tab.target.kind {
                ObjectKind::Table { name } => {
                    database
                        .tables
                        .iter()
                        .filter(|table| table.name == *name && table.local && !table.system)
                        .count()
                        == 1
                }
                _ => false,
            }
            && self.objects.revision == Some(database.document_revision)
            && !self.objects.entries.is_empty()
            && self.objects.entries.len() <= MAX_OBJECTS
            && self
                .objects
                .entries
                .iter()
                .any(|entry| entry.tab == request.target.tab)
            && !self.field_rename_close_open()
            && !self.field_addition_open()
            && !self.field_addition_modal_open()
            && !self.database_options_pending()
            && replacement::ready(self)
            && replacement::fits(self, database) == Some(true)
    }

    /// Native branch prerequisite: actual known AFTER ACK/release, exact accepted
    /// rename request/review/save identity and successful complete FieldRenamedReopen
    /// read. Unchanged branch prerequisite: a genuine unchanged runtime review
    /// consumed into its complete-image readonly witness, then successful read.
    /// Neither a Database DTO nor an observed bool supplies those permissions.
    ///
    /// Keep all tab IDs/order, source-free scratch SQL and clean preferences.
    /// Invalidate native rows/handles/results. Opaque filter/sort definitions are
    /// host-owned: revalidate them against the fresh schema, never rewrite SQL or
    /// quietly discard a failed restoration. Refusal leaves current UI unchanged.
    pub fn resolve_field_rename_after_reopen(
        &mut self,
        request: &FieldRenameRequest,
        identity: FieldRenameSaveIdentity,
        database: Database,
    ) -> bool {
        if !self.matches_field_rename_save(request, identity)
            || !self.can_rebind_field_rename_layout(request)
            || self
                .database
                .as_ref()
                .is_none_or(|old| !catalog::matches(old, &database))
            || replacement::fits(self, &database) != Some(true)
        {
            return false;
        }
        let Some(prepared) = replacement::prepare(self, &database, replacement::Transfer::KeepAll)
        else {
            return false;
        };

        replacement::publish(self, prepared, database, catalog::Change::Unchanged)
    }
}
