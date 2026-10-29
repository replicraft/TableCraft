//! Existing record patches and multiple growing index owners form one candidate.
mod authority;

use super::FixedColumnUpdates;
use crate::{
    Ace16WritePreview, EditError, IndexCheckStatus, InspectError, NativeImage, WriteBlocker,
    check_table,
    header::check,
    inspect_catalog,
    table::{
        indexes::{
            SplitIndexPlan, fixed_batch_split_patches,
            mutation::split::fixed_batch::{self, Input},
        },
        mutation::{append::owners::OwnerContext, index_append, patch::spans},
    },
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

pub(super) struct Operation<'a, 'u> {
    pub tables: &'a [super::plan::Table<'u>],
    pub groups: &'a [FixedColumnUpdates<'u>],
    pub fields: &'a [spans::Patch],
    pub changed_rows: usize,
}

impl NativeImage {
    /// Preview an Int32 batch across tables with nullable single/tuple index growth.
    ///
    /// Actual aliases share one physical transform and append ledger. Existing
    /// forwarded logical roots retain identity while exclusive owned final
    /// records receive payload/mask patches. Secondary IgnoreNULL follows the
    /// original-association history policy; Required precedes omission and
    /// ambiguous family reinsertion remains refused. Index leaf/internal splits
    /// reuse qualified existing growth rules. Depletion needing contraction is
    /// separate work; records do not resize and no DATA page is allocated here.
    ///
    /// This allocation profile accepts Int32 assignments and affected Int32
    /// index components. Multi-table allocation additionally certifies every
    /// physical owner under that profile before the shared cursor advances.
    /// Wider indexed carriers remain unfinished. The existing fitting, slot, cascade
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
    if operation.tables.len() == 1 {
        let table = &operation.tables[0];
        let plan = fixed_batch_split_patches(
            &image,
            table.before.schema(),
            table.before.indexes(),
            &table.keys,
            budget,
            cancelled,
        )?;
        return finish_plan(image, operation, &plan, budget, cancelled);
    }

    budget
        .charge_allocation(operation.tables.len() as u64, size_of::<Input<'_>>() as u64)
        .map_err(InspectError::from)?;
    let mut inputs = Vec::new();
    inputs
        .try_reserve_exact(operation.tables.len())
        .map_err(|_| InspectError::Allocation)?;
    for table in operation.tables {
        check(budget, cancelled)?;
        inputs.push(Input {
            checked: &table.before,
            updates: &table.keys,
        });
    }
    let plan = fixed_batch::multiple(&image, &inputs, budget, cancelled)?;
    finish_plan(image, operation, &plan, budget, cancelled)
}

fn finish_plan(
    image: NativeImage,
    operation: Operation<'_, '_>,
    plan: &SplitIndexPlan<impl OwnerContext>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    budget
        .charge_allocation(
            operation.tables.len() as u64,
            size_of::<&crate::TableCheck>() as u64,
        )
        .map_err(InspectError::from)?;
    let mut checked_tables = Vec::new();
    checked_tables
        .try_reserve_exact(operation.tables.len())
        .map_err(|_| InspectError::Allocation)?;
    checked_tables.extend(operation.tables.iter().map(|table| &table.before));
    index_append::qualify(&image, plan)?;
    let after = index_append::compose(&image, operation.fields, plan, budget, cancelled)?;
    let certified_rows = authority::prove(
        &image,
        &checked_tables,
        operation.groups,
        plan,
        &after,
        budget,
        cancelled,
    )?;
    if certified_rows != operation.changed_rows {
        return Err(EditError::Preservation);
    }
    let scope = image.catalog().catalog().scope();
    inspect_catalog(&mut Cursor::new(&after), scope, budget, cancelled)?;
    for table in operation.tables {
        let checked = check_table(
            &mut Cursor::new(&after),
            scope,
            table.before.schema().name(),
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
    }
    check(budget, cancelled)?;
    Ok(Ace16WritePreview::from_candidate(
        image.bytes,
        after,
        certified_rows,
    ))
}
