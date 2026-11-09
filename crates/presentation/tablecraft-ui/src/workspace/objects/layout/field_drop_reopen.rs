//! DROP-specific UI publication after the host verifies its complete native witness.
use super::{catalog, replacement};
use crate::{
    Database,
    workspace::{
        State,
        field_drop::{FieldDropRequest, FieldDropSaveIdentity},
        objects::{MAX_OBJECTS, ObjectKind},
    },
};

impl State {
    /// Source-neutral layout admission, not native review or receipt authority.
    pub fn can_rebind_field_drop_layout(&self, request: &FieldDropRequest) -> bool {
        let Some(database) = &self.database else {
            return false;
        };
        self.field_drop_request_is_current(request)
            && request.target.tab.target.document_revision == database.document_revision
            && matches!(&request.target.tab.target.kind, ObjectKind::Table { name }
                if database.tables.iter().filter(|table| table.name == *name && table.local && !table.system).count() == 1)
            && self.objects.revision == Some(database.document_revision)
            && !self.objects.entries.is_empty()
            && self.objects.entries.len() <= MAX_OBJECTS
            && self
                .objects
                .entries
                .iter()
                .any(|entry| entry.tab == request.target.tab)
            && !self.field_drop_close_open()
            && !self.field_addition_open()
            && !self.field_rename_open()
            && !self.database_options_pending()
            && replacement::ready(self)
            && replacement::fits(self, database) == Some(true)
    }

    /// Host prerequisites: exact consuming DROP review/save identity, real known
    /// AFTER ACK/release, then successful full FieldDroppedReopen witness read.
    /// A Database DTO or Saved message alone grants none of those permissions.
    /// Preserve tabs/scratch/preferences; invalidate native caches only on atomic
    /// publication. Filters/sorts are host-owned and must be freshly validated.
    pub fn resolve_field_drop_after_reopen(
        &mut self,
        request: &FieldDropRequest,
        identity: FieldDropSaveIdentity,
        database: Database,
    ) -> bool {
        if !self.matches_field_drop_save(request, identity)
            || !self.field_drop_reopening(identity)
            || !self.can_rebind_field_drop_layout(request)
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
        if !replacement::publish(self, prepared, database, catalog::Change::Unchanged) {
            return false;
        }
        self.finish_field_drop_publication();
        true
    }
}
