//! One fitting Short Text replacement profile with retained advanced payloads.
mod growing;
pub(super) mod proof;
mod stage;

use super::{copy, source};
use crate::{
    Ace16WritePreview, EditError, InspectError, NativeImage, SlotScalarUpdate,
    header::check,
    table::mutation::{patch::BytePatch, preview},
};
use tablecraft_core::limits::ResourceBudget;

impl NativeImage {
    /// Preview one ordinary, nonindexed, calculation-independent Short Text
    /// target in a preservation-qualified mixed schema. Each changed record must
    /// fit its original physical length; no-op representations stay exact.
    /// This separate candidate grants no normal-save, journal or macro authority.
    pub fn preview_ace16_preserved_text_updates(
        self,
        table: &str,
        column: &str,
        updates: &[SlotScalarUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        let _span = tracing::debug_span!("native.edit.preserved_text_preview").entered();
        check(budget, cancelled)?;
        preview::qualify(&self)?;
        if updates.len() > Self::MAX_FIXED_BATCH_ASSIGNMENTS {
            return Err(InspectError::StructureLimit.into());
        }
        let original = source::capture(&self, table, budget, cancelled)?;
        let (patches, changed) = stage::prepare(&original, column, updates, budget, cancelled)?;
        let mut after = copy(self.as_bytes(), budget, cancelled)?;
        for patch in patches {
            check(budget, cancelled)?;
            let end = patch
                .offset()
                .checked_add(patch.before().len())
                .ok_or(EditError::Preservation)?;
            let bytes = after
                .get_mut(patch.offset()..end)
                .ok_or(EditError::Preservation)?;
            if bytes != patch.before() {
                return Err(EditError::Preservation);
            }
            bytes.copy_from_slice(patch.after());
        }
        proof::prove(
            &self,
            table,
            column,
            updates,
            proof::Observed {
                bytes: &after,
                changed_rows: changed,
            },
            budget,
            cancelled,
        )?;
        let candidate = self.recapture_fixed_candidate(&after, budget, cancelled)?;
        source::capture(&candidate, table, budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(Ace16WritePreview::from_candidate(
            self.bytes, after, changed,
        ))
    }
}

#[cfg(test)]
mod genuine_sources;
