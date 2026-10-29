//! Bind allocating UPDATE authority to original groups and immutable records.
mod cells;
mod tables;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod multitable_tests;

use crate::{
    EditError, FixedColumnUpdates, InspectError, NativeImage, TableCheck, check_table_constraints,
    header::check,
    table::{
        indexes::{SplitIndexPlan, mutation::split::fixed_batch::Input},
        mutation::{append::owners::OwnerContext, gates, preservation},
    },
};
use std::{io::Cursor, ops::Range};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn prove<O: OwnerContext>(
    image: &NativeImage,
    checked: &[&TableCheck],
    groups: &[FixedColumnUpdates<'_>],
    plan: &SplitIndexPlan<O>,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    check(budget, cancelled)?;
    let tables = tables::derive(image, checked, groups, after, budget, cancelled)?;
    let mut inputs = Vec::new();
    for table in &tables {
        check(budget, cancelled)?;
        push(
            &mut inputs,
            Input {
                checked: &table.checked,
                updates: &table.fields.keys,
            },
            budget,
        )?;
    }
    let mut spans = plan.audit_fixed_batches(image, &inputs, after, budget, cancelled)?;
    let mut changed = 0usize;
    for table in &tables {
        check(budget, cancelled)?;
        changed = changed
            .checked_add(table.fields.changed)
            .ok_or(InspectError::StructureLimit)?;
        for byte in &table.fields.bytes {
            check(budget, cancelled)?;
            if after.get(byte.offset) != Some(&byte.after) {
                return Err(EditError::Preservation);
            }
            push(&mut spans, byte.offset..byte.offset + 1, budget)?;
        }
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
    // Constraints are evaluated on the single fully composed image. The table
    // checker includes incoming relationships from untouched child tables.
    for table in &tables {
        gates::qualified(&check_table_constraints(
            &mut Cursor::new(after),
            image.catalog().catalog().scope(),
            table.checked.schema().name(),
            budget,
            cancelled,
        )?)?;
    }
    check(budget, cancelled)?;
    Ok(changed)
}

fn push<T>(items: &mut Vec<T>, item: T, budget: &mut ResourceBudget) -> Result<(), EditError> {
    budget
        .charge_allocation(1, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    items.try_reserve(1).map_err(|_| InspectError::Allocation)?;
    items.push(item);
    Ok(())
}
