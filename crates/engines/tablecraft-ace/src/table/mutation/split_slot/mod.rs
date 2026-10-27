//! Exact existing-slot replacements with independently certified index growth.
mod authority;
mod finish;
#[cfg(test)]
mod tests;

use super::{EditError, SlotScalarUpdate, gates, preview, slot};
use crate::{
    Ace16WritePreview, InspectError, NativeImage, TableCheck,
    header::check,
    limits::MAX_LIVE_ROWS,
    table::indexes::{SplitIndexPlan, index_split_slot_patches, qualify_split_slot_indexes},
};
use tablecraft_core::limits::ResourceBudget;

struct Operation<'a> {
    table: &'a str,
    column: &'a str,
    requests: &'a [SlotScalarUpdate],
}

struct Staged<'a> {
    structural: TableCheck,
    column: usize,
    slots: slot::stage::Staged<'a>,
    plan: SplitIndexPlan,
}

impl NativeImage {
    /// Preview Short Text/Binary replacements within their exact existing slots,
    /// allowing qualified single-key General/Binary indexes to split recursively.
    ///
    /// Existing NULL/empty semantics and exact expected-value preconditions remain
    /// in force. Records never resize or move. Affected tuple, French/unknown
    /// collation, Memo, ignore-NULL and unsupported overflow profiles remain
    /// refused. Original gates and complete final constraints still apply.
    ///
    /// This explicit engineering candidate is non-journalable. It does not widen
    /// existing slot/page/variable previews, normal saves or Access qualification.
    pub fn preview_ace16_split_slot_updates(
        self,
        table: &str,
        column: &str,
        requests: &[SlotScalarUpdate],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Ace16WritePreview, EditError> {
        let _span = tracing::debug_span!(
            "native.edit.preview.split_slot",
            assignments = requests.len()
        )
        .entered();
        let operation = Operation {
            table,
            column,
            requests,
        };
        let (structural, position) = preflight(&self, &operation, budget, cancelled)?;
        let staged = stage(&self, &operation, structural, position, budget, cancelled)?;

        finish::finish(self, &operation, &staged, budget, cancelled)
    }
}

fn preflight(
    image: &NativeImage,
    operation: &Operation<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(TableCheck, usize), EditError> {
    check(budget, cancelled)?;
    preview::qualify(image)?;
    if operation.requests.len() > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit.into());
    }

    let structural = gates::before_slot(image, operation.table, budget, cancelled)?;
    let column = slot::selected(structural.schema(), operation.column)?;
    qualify_split_slot_indexes(
        structural.schema(),
        structural.indexes(),
        column,
        budget,
        cancelled,
    )?;

    Ok((structural, column))
}

fn stage<'a>(
    image: &NativeImage,
    operation: &Operation<'a>,
    structural: TableCheck,
    column: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged<'a>, EditError> {
    let slots = slot::stage::stage(
        image,
        &structural,
        column,
        operation.requests,
        budget,
        cancelled,
    )?;

    let keys = slots.keys(column, budget, cancelled)?;
    let plan = index_split_slot_patches(
        image,
        structural.schema(),
        structural.indexes(),
        column,
        &keys,
        budget,
        cancelled,
    )?;

    Ok(Staged {
        structural,
        column,
        slots,
        plan,
    })
}
