//! Atomic SQL-origin layout publication after a host-verified native reopen.
use super::{catalog, replacement};
use crate::workspace::State;
use crate::{Database, QueryActionEffect, QuerySchemaEffect, QuerySchemaPublication};

impl State {
    /// Preserve the accepted SQL tab/text while replacing only freshly verified
    /// catalog/read owners. The host must first ACK the matching native actor and
    /// successfully run that capability's exact guarded reopen command.
    ///
    /// A refusal leaves the current UI snapshot and the caller's read-only
    /// witness intact; it supplies no native write or retry permission.
    pub fn resolve_query_schema_after_reopen(
        &mut self,
        witness: &QuerySchemaPublication,
        database: Database,
    ) -> bool {
        if !witness.current(self)
            || !replacement::ready(self)
            || !self.query_schema_target_closed(witness.review())
            || replacement::fits(self, &database) != Some(true)
        {
            return false;
        }
        let QueryActionEffect::Schema(effect) = &witness.review().effect else {
            return false;
        };
        let change = match effect {
            QuerySchemaEffect::CreateTable { .. } => {
                catalog::Change::TableCreated(&witness.review().table)
            }
            QuerySchemaEffect::DropTable { .. } => {
                catalog::Change::TableDropped(&witness.review().table)
            }
            QuerySchemaEffect::DropColumn { .. } => catalog::Change::Unchanged,
        };
        if self
            .database
            .as_ref()
            .is_none_or(|old| !catalog::matches_change(old, &database, change))
        {
            return false;
        }
        let Some(prepared) = replacement::prepare(self, &database, replacement::Transfer::KeepAll)
        else {
            return false;
        };
        if !replacement::publish(self, prepared, database, change) {
            return false;
        }
        self.schema_reopen = None;
        true
    }
}
