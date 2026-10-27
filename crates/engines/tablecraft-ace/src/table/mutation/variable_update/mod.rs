//! Separate alternative-owned-page and represented-EOF forwarding previews.
mod audit;
mod authority;
mod finish;
mod map_audit;
mod maps;
mod placement;
mod record_proof;

use super::{
    Ace16WritePreview, EditError, SlotScalarUpdate, WriteBlocker, gates, page_update::stage,
    preview, slot::storage,
};
use crate::{
    InspectError, NativeImage,
    header::check,
    limits::MAX_LIVE_ROWS,
    table::indexes::{index_slot_patches, qualify_slot_indexes},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

impl NativeImage {
    /// Preview variable Short Text/Binary replacements with checked alternate
    /// owned-page placement or bounded EOF data-page allocation. Only already
    /// represented ownership/candidate/global bitmap cells may change. Original
    /// logical row/index references remain stable; no map growth or arbitrary
    /// in-file free-page reuse occurs.
    ///
    /// This separate non-journalable engineering service retains all column,
    /// macro, index, constraint and preservation gates. The root-page-only and
    /// exact-span APIs keep their existing placement/admission contracts.
    pub fn preview_ace16_variable_updates(
        self,
        table: &str,
        column: &str,
        updates: &[SlotScalarUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        let _span =
            tracing::debug_span!("native.edit.preview.variable", assignments = updates.len())
                .entered();
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
        let placement = placement::Plan::capture(&self, schema, budget, cancelled)?;
        let staged = stage::build_with(
            &self,
            &structural,
            position,
            updates,
            placement,
            budget,
            cancelled,
        )?;
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
        finish::finish(
            self,
            &authority::Operation::new(table, column, updates),
            &staged.plan,
            &indexes,
            staged.changed_rows(),
            budget,
            cancelled,
        )
    }
}
