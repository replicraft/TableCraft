//! Existing record patches and multiple growing index owners form one candidate.
mod authority;

use super::FixedColumnUpdates;
use crate::{
    Ace16WritePreview, EditError, IndexCheckStatus, NativeImage, TableCheck, WriteBlocker,
    check_table, check_table_constraints,
    header::check,
    inspect_catalog,
    table::{
        indexes::{IndexKeyUpdate, fixed_batch_split_patches},
        mutation::{gates, index_append, patch::spans},
    },
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

pub(super) struct Operation<'a, 'u> {
    pub checked: &'a TableCheck,
    pub groups: &'a [FixedColumnUpdates<'u>],
    pub fields: &'a [spans::Patch],
    pub keys: &'a [IndexKeyUpdate],
    pub changed_rows: usize,
}

impl NativeImage {
    /// Preview one table's Int32 batch with nullable single/tuple index growth.
    ///
    /// Actual aliases share one physical transform and append ledger. Existing
    /// forwarded logical roots retain identity while exclusive owned final
    /// records receive payload/mask patches. Secondary IgnoreNULL follows the
    /// original-association history policy; Required precedes omission and
    /// ambiguous family reinsertion remains refused. Index leaf/internal splits
    /// reuse qualified existing growth rules. Depletion needing contraction is
    /// separate work; records do not resize and no DATA page is allocated here.
    ///
    /// This initial allocation profile accepts one table and Int32 assignments
    /// with affected Int32 index components. Multiple-table allocation and wider
    /// indexed carriers remain unfinished. The existing fitting, slot, cascade
    /// and normal save services retain their separate admission. This returns
    /// only a non-journalable engineering candidate, not native Access or macro
    /// qualification, and planning never writes a source.
    pub fn preview_ace16_split_batch_updates(
        self,
        groups: &[FixedColumnUpdates<'_>],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        self.preview_fixed_batch_inner::<true, true>(groups, budget, cancelled)
    }
}

pub(super) fn finish(
    image: NativeImage,
    operation: Operation<'_, '_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    check(budget, cancelled)?;
    let schema = operation.checked.schema();
    let plan = fixed_batch_split_patches(
        &image,
        schema,
        operation.checked.indexes(),
        operation.keys,
        budget,
        cancelled,
    )?;
    index_append::qualify(&image, &plan)?;
    let after = index_append::compose(&image, operation.fields, &plan, budget, cancelled)?;
    let certified_rows = authority::prove(
        &image,
        operation.checked,
        operation.groups,
        &plan,
        &after,
        budget,
        cancelled,
    )?;
    if certified_rows != operation.changed_rows {
        return Err(EditError::Preservation);
    }
    let scope = image.catalog().catalog().scope();
    inspect_catalog(&mut Cursor::new(&after), scope, budget, cancelled)?;
    let checked = check_table(
        &mut Cursor::new(&after),
        scope,
        schema.name(),
        budget,
        cancelled,
    )?;
    if checked
        .indexes()
        .iter()
        .any(|index| index.status() != IndexCheckStatus::Checked)
    {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    gates::qualified(&check_table_constraints(
        &mut Cursor::new(&after),
        scope,
        schema.name(),
        budget,
        cancelled,
    )?)?;
    check(budget, cancelled)?;
    Ok(Ace16WritePreview::from_candidate(
        image.bytes,
        after,
        certified_rows,
    ))
}
