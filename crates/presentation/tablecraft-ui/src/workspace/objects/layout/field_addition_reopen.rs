//! Distinct atomic publication after a host-proven field schema replacement.
#[cfg(test)]
mod tests;

use super::{catalog, replacement};
use crate::{
    Database,
    workspace::{
        State,
        field_addition::{FieldAdditionRequest, FieldAdditionSaveIdentity},
        objects::{MAX_OBJECTS, ObjectKind},
    },
};

impl State {
    /// Structural preflight for this exact retained field-addition naming request.
    /// Global busy and this operation's own status are allowed. This is neither
    /// native source admission nor evidence that a future reopen will succeed.
    pub fn can_rebind_field_addition_layout(&self, request: &FieldAdditionRequest) -> bool {
        let Some(database) = &self.database else {
            return false;
        };
        let Some(active) = self.objects.active else {
            return false;
        };
        !self.field_rename_open()
            && !self.field_rename_status_open()
            && self.field_addition_request_is_current(request)
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
            && replacement::ready(self)
            && replacement::fits(self, database) == Some(true)
    }

    /// Publish only after the host acknowledged a known AFTER field-addition
    /// receipt AND successfully reopened its complete AddedColumnReopen witness.
    /// The witness must belong to this exact request/review/save identity and
    /// prove the same-name column addition, preserving other object definitions.
    /// These are explicit caller preconditions, never inferred from Database.
    ///
    /// This separate schema route does not accept query-only publication proof.
    /// It preserves tab identities/order, scratch SQL, clean saved SQL text and
    /// source-neutral preferences, invalidating native results/handles/pending
    /// authority. Host-owned filter/sort definitions require fresh validation.
    /// The field-addition model/status remain retained until the host clears them after
    /// successful publication. A refusal leaves current state/text unchanged.
    pub fn resolve_field_addition_after_reopen(
        &mut self,
        request_id: u64,
        request: &FieldAdditionRequest,
        review_id: u64,
        save_id: u64,
        database: Database,
    ) -> bool {
        let identity = FieldAdditionSaveIdentity {
            request_id,
            review_id,
            save_id,
        };
        if !self.matches_field_addition_save(request, identity)
            || !self.can_rebind_field_addition_layout(request)
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
