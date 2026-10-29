//! Bind allocating UPDATE authority to original groups and immutable records.
mod cells;

#[cfg(test)]
mod tests;

use crate::{
    EditError, FixedColumnUpdates, InspectError, NativeImage, TableCheck, WriteBlocker,
    header::check,
    table::{
        indexes::SplitIndexPlan,
        mutation::{gates, preservation},
    },
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn prove(
    image: &NativeImage,
    checked: &TableCheck,
    groups: &[FixedColumnUpdates<'_>],
    plan: &SplitIndexPlan,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    check(budget, cancelled)?;
    let table = checked.schema().name();
    if groups.len() > NativeImage::MAX_FIXED_BATCH_GROUPS {
        return Err(InspectError::StructureLimit.into());
    }
    for (position, group) in groups.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, position as u64)
            .map_err(InspectError::from)?;
        if !group.table().eq_ignore_ascii_case(table) {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        if groups[..position]
            .iter()
            .any(|other| other.column().eq_ignore_ascii_case(group.column()))
        {
            return Err(EditError::Conflict);
        }
    }
    let original = gates::before_fixed_update(image, table, budget, cancelled)?;
    if !original.schema().same_definition(checked.schema())
        || original.schema().object_id() != checked.schema().object_id()
    {
        return Err(EditError::Preservation);
    }
    let fields = cells::derive(image, original.schema(), groups, after, budget, cancelled)?;
    let mut spans =
        plan.audit_fixed_batch(image, &original, &fields.keys, after, budget, cancelled)?;
    for byte in &fields.bytes {
        check(budget, cancelled)?;
        if after.get(byte.offset) != Some(&byte.after) {
            return Err(EditError::Preservation);
        }
        push(&mut spans, byte.offset..byte.offset + 1, budget)?;
    }
    let levels = spans.len().checked_ilog2().unwrap_or(0) as usize + 1;
    budget
        .charge(Resource::Steps, spans.len().saturating_mul(levels) as u64)
        .map_err(InspectError::from)?;
    spans.sort_unstable_by_key(|span| span.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for span in spans {
        check(budget, cancelled)?;
        if let Some(last) = merged.last_mut()
            && span.start <= last.end
        {
            last.end = last.end.max(span.end);
        } else {
            push(&mut merged, span, budget)?;
        }
    }
    let prefix = after
        .get(..image.as_bytes().len())
        .ok_or(EditError::Preservation)?;
    preservation::prove(image.as_bytes(), prefix, merged, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(fields.changed)
}

fn push<T>(items: &mut Vec<T>, item: T, budget: &mut ResourceBudget) -> Result<(), EditError> {
    budget
        .charge_allocation(1, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    items.try_reserve(1).map_err(|_| InspectError::Allocation)?;
    items.push(item);
    Ok(())
}
