//! Separate ordinary Memo/OLE replacement preview and its column-owned storage.
mod admission;
mod audit;
mod authority;
mod encode;
mod finish;
mod map_audit;
mod maps;
mod ownership;
mod placement;
mod stage;
#[cfg(test)]
mod tests;
mod value;

use super::{Ace16WritePreview, EditError, SlotScalarUpdate};
use crate::{InspectError, NativeImage, header::check, limits::MAX_LIVE_ROWS};
use tablecraft_core::limits::ResourceBudget;

struct Operation<'a> {
    table: &'a str,
    column: &'a str,
    updates: &'a [SlotScalarUpdate],
}

impl NativeImage {
    /// Preview ordinary raw Memo/OLE replacements with qualified column-owned
    /// LVAL appends and represented EOF allocation. Old chunk rows, including
    /// newly orphaned chains, remain unchanged and allocated. Parent descriptor
    /// growth uses stable-root ordinary forwarding through the same EOF ledger.
    ///
    /// This separate engineering candidate has no journal/commit conversion.
    /// Indexed/extended long fields, unknown column maps, shared storage and
    /// unavailable constraint/macro behavior retain explicit refusal boundaries.
    /// Existing Short Text/Binary APIs keep their original admissions.
    pub fn preview_ace16_long_value_updates(
        self,
        table: &str,
        column: &str,
        updates: &[SlotScalarUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        let _span = tracing::debug_span!(
            "native.edit.preview.long_value",
            assignments = updates.len()
        )
        .entered();
        check(budget, cancelled)?;
        if updates.len() > MAX_LIVE_ROWS {
            return Err(InspectError::StructureLimit.into());
        }
        let operation = Operation {
            table,
            column,
            updates,
        };
        let prepared = admission::prepare(&self, table, column, budget, cancelled)?;
        let staged = stage::build(&self, &prepared, updates, budget, cancelled)?;
        finish::finish(self, &operation, &staged, budget, cancelled)
    }
}
