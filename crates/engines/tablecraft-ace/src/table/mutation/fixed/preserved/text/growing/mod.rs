//! Explicit DATA placement for one preserved, ordinary Short Text target.
mod finish;
mod proof;
mod stage;
#[cfg(test)]
mod tests;

use super::super::source;
use crate::{
    Ace16WritePreview, EditError, InspectError, NativeImage, SlotScalarUpdate, header::check,
    table::mutation::preview,
};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Preview one unindexed, calculation-independent Short Text target using
    /// qualified stable-root placement in a preserved mixed schema. Existing
    /// owned pages and represented EOF allocation are available; index and map
    /// storage growth remain refused. This separate engineering candidate grants
    /// no normal-save, journal or macro permission.
    pub fn preview_ace16_preserved_text_growth(
        self,
        table: &str,
        column: &str,
        updates: &[SlotScalarUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        let _span = tracing::debug_span!("native.edit.preserved_text_growth").entered();
        check(budget, cancelled)?;
        preview::qualify(&self)?;
        if updates.len() > Self::MAX_FIXED_BATCH_ASSIGNMENTS {
            return Err(InspectError::StructureLimit.into());
        }

        let original = source::capture(&self, table, budget, cancelled)?;
        let (placement, changed) =
            stage::prepare(&self, &original, column, updates, budget, cancelled)?;

        finish::candidate(
            self,
            table,
            column,
            updates,
            finish::Observed {
                placement: &placement,
                changed_rows: changed,
            },
            budget,
            cancelled,
        )
    }
}
