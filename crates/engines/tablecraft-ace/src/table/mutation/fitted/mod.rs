//! One original-image fixed/variable statement, with no page allocation.
pub(in crate::table) mod growing;
mod record_proof;
mod request;
mod serialization;
mod stage;

pub use request::FittedColumnUpdates;

use super::{Ace16WritePreview, EditError, gates, preview};
use crate::{NativeImage, table::indexes::index_row_patches};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Prepare fixed fields plus one Short Text/Binary field in one original
    /// table. The final record must fit its existing physical capacity. Every
    /// expected cell refers to this image; all final keys and constraints are
    /// checked jointly. Text prefers declared Unicode compression when its units
    /// and existing capacity allow it, including the marker-only empty form;
    /// legal raw UTF16 remains available when its smaller form alone fits.
    /// No DATA/INDEX allocation, forwarding placement, save,
    /// journal or macro permission is introduced by this engineering preview.
    pub fn preview_ace16_fitted_updates(
        self,
        table: &str,
        groups: &[FittedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        let _span = tracing::debug_span!("native.edit.fitted_preview").entered();
        request::check(budget, cancelled)?;
        preview::qualify(&self)?;
        let original = gates::before_slot(&self, table, budget, cancelled)?;
        let staged = stage::build(&self, &original, groups, budget, cancelled)?;
        let keys = staged.keys(budget, cancelled)?;
        let indexes = index_row_patches(
            &self,
            original.schema(),
            original.indexes(),
            &staged.affected,
            &keys,
            budget,
            cancelled,
        )?;

        // No broad RecordPatch is handed to the final composer until fresh
        // source/request authority proves full rows AND joint key maintenance.
        let changed =
            record_proof::prove(&self, table, groups, &staged, &indexes, budget, cancelled)?;
        let candidate =
            self.finish_updates(table, &staged.records, &indexes, changed, budget, cancelled)?;
        request::check(budget, cancelled)?;
        Ok(Ace16WritePreview::from_plan(candidate))
    }
}

#[cfg(test)]
mod tests;
