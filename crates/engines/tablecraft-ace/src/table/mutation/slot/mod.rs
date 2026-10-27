//! Existing-slot Text/Binary changes with mandatory index and constraint proof.
pub(super) mod stage;
pub(super) mod storage;
mod update;

use super::{Ace16WritePreview, EditError, WriteBlocker, gates, preview};
use crate::{
    InspectError, NativeImage, TableSchema,
    header::check,
    limits::MAX_LIVE_ROWS,
    table::indexes::{index_slot_patches, qualify_slot_indexes},
};
use tablecraft_core::limits::ResourceBudget;
pub use update::SlotScalarUpdate;

impl NativeImage {
    /// Preview exact expected-value Short Text/Binary changes within each
    /// existing physical variable slot. Offset directories, record sizes,
    /// allocation and index topology never grow. Indexed changes use genuine
    /// complete key re-encoding and full before/after constraint checks.
    ///
    /// This engineering-only candidate neither qualifies macros nor enables a
    /// normal backend save, journal operation or Access compatibility claim.
    pub fn preview_ace16_slot_updates(
        self,
        table: &str,
        column: &str,
        updates: &[SlotScalarUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        let _span =
            tracing::debug_span!("native.edit.preview.slot", assignments = updates.len()).entered();
        check(budget, cancelled)?;
        preview::qualify(&self)?;
        if updates.len() > MAX_LIVE_ROWS {
            return Err(InspectError::StructureLimit.into());
        }
        let structural = gates::before_slot(&self, table, budget, cancelled)?;
        let schema = structural.schema();
        let position = selected(schema, column)?;

        qualify_slot_indexes(schema, structural.indexes(), position, budget, cancelled)?;
        let staged = stage::stage(&self, &structural, position, updates, budget, cancelled)?;
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
        self.finish_updates(
            table,
            &staged.patches,
            &indexes,
            staged.changed.len(),
            budget,
            cancelled,
        )
        .map(Ace16WritePreview::from_plan)
    }
}

pub(super) fn selected(schema: &TableSchema, column: &str) -> Result<usize, EditError> {
    let position = schema
        .columns
        .iter()
        .position(|field| field.name().eq_ignore_ascii_case(column))
        .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    let field = &schema.columns[position];
    storage::qualify(field)?;
    if schema.columns.iter().enumerate().any(|(other, candidate)| {
        other != position && !candidate.fixed() && candidate.field.variable == field.field.variable
    }) {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }

    Ok(position)
}
