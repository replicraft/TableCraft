//! Bounded existing-page forwarding with stable logical roots and full checks.
mod audit;
mod forward;
mod record;
mod roots;
mod space;
mod stage;
#[cfg(test)]
mod tests;

use super::{
    Ace16WritePreview, EditError, SlotScalarUpdate, WriteBlocker, gates, preview, slot::storage,
};
use crate::{
    InspectError, NativeImage,
    header::check,
    limits::MAX_LIVE_ROWS,
    table::indexes::{index_slot_patches, qualify_slot_indexes},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

impl NativeImage {
    /// Preview Short Text/Binary changes using fitted existing records or new
    /// hidden last slots on the same root page. Original logical row/index
    /// references stay stable. No page allocation or alternative-page fallback
    /// occurs; unavailable candidate hints, slot IDs or bytes refuse atomically.
    ///
    /// Existing forwarded records use checked exclusive physical targets. This
    /// separate engineering API retains all macro/constraint/preservation gates;
    /// it neither relaxes the exact-span slot API nor enables normal ACE saves.
    pub fn preview_ace16_page_updates(
        self,
        table: &str,
        column: &str,
        updates: &[SlotScalarUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        let _span =
            tracing::debug_span!("native.edit.preview.page", assignments = updates.len()).entered();
        check(budget, cancelled)?;
        preview::qualify(&self)?;
        if updates.len() > MAX_LIVE_ROWS {
            return Err(InspectError::StructureLimit.into());
        }
        let structural = gates::before_slot(&self, table, budget, cancelled)?;
        let schema = structural.schema();
        let position = schema
            .columns
            .iter()
            .position(|field| field.name().eq_ignore_ascii_case(column))
            .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
        let field = &schema.columns[position];
        storage::qualify(field)?;
        if schema.columns.iter().enumerate().any(|(other, candidate)| {
            other != position
                && !candidate.fixed()
                && candidate.field.variable == field.field.variable
        }) {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        qualify_slot_indexes(schema, structural.indexes(), position, budget, cancelled)?;
        for update in updates {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, (2 * field.storage_length()) as u64)
                .map_err(InspectError::from)?;
            storage::validate(field, update.expected())?;
            storage::validate(field, update.value())?;
        }

        let staged = stage::build(&self, &structural, position, updates, budget, cancelled)?;
        staged.audit(&self, &structural, position, budget, cancelled)?;
        let keys = staged.keys(position, budget, cancelled)?;
        let indexes = index_slot_patches(
            &self,
            schema,
            structural.indexes(),
            position,
            &keys,
            budget,
            cancelled,
        )?;
        let plan = self.finish_updates(
            table,
            &staged.plan.pages,
            &indexes,
            staged.changed_rows(),
            budget,
            cancelled,
        )?;
        audit::final_pages(&staged.plan, plan.after_bytes(), budget, cancelled)?;
        staged
            .space
            .audit(plan.before_bytes(), plan.after_bytes(), budget, cancelled)?;
        check(budget, cancelled)?;
        Ok(Ace16WritePreview::from_plan(plan))
    }
}
