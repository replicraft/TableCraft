//! Exact closed-table catalog publication after host-owned known-AFTER verification.
use super::{catalog, replacement};
use crate::{
    Database,
    workspace::{ObjectKind, State, TableDropRequest, TableDropSaveIdentity},
};

impl State {
    /// Source-neutral layout admission; this does not grant native save authority.
    pub fn can_rebind_table_drop_layout(&self, request: &TableDropRequest) -> bool {
        let Some(name) = request.table_name() else {
            return false;
        };
        self.table_drop_request() == Some(request)
            && self.database.as_ref().is_some_and(|database| {
                database.document_revision == request.target.document_revision
                    && database
                        .tables
                        .iter()
                        .filter(|table| table.name == name)
                        .count()
                        == 1
                    && database
                        .tables
                        .iter()
                        .any(|table| table.name == name && table.local && !table.system)
                    && self.objects.revision == Some(database.document_revision)
            })
            && !self.has_open_native_objects()
            && replacement::ready(self)
    }

    /// The host must first consume the exact native review, ACK known AFTER,
    /// release its resources and verify a full table-dropped reopen witness.
    /// This DTO gate preserves SQL drafts and tab IDs; it proves no native bytes.
    pub fn resolve_table_drop_after_reopen(
        &mut self,
        request: &TableDropRequest,
        identity: TableDropSaveIdentity,
        database: Database,
    ) -> bool {
        let Some(name) = request.table_name() else {
            return false;
        };
        let change = catalog::Change::TableDropped(name);
        if !self.matches_table_drop_save(request, identity)
            || !self.table_drop_reopening(identity)
            || !self.can_rebind_table_drop_layout(request)
            || self
                .database
                .as_ref()
                .is_none_or(|old| !catalog::matches_change(old, &database, change))
            || replacement::fits(self, &database) != Some(true)
        {
            return false;
        }
        let Some(mut prepared) =
            replacement::prepare(self, &database, replacement::Transfer::KeepAll)
        else {
            return false;
        };
        if prepared.objects.navigation.as_ref().is_some_and(|target| {
            matches!(&target.kind, ObjectKind::Table { name: selected } if selected == name)
        }) {
            prepared.objects.navigation = None;
        }
        if !replacement::publish(self, prepared, database, change) {
            return false;
        }
        self.finish_table_drop_publication();
        true
    }
}
