//! Original-only fitting INDEX tree reconstruction for normal BeforeChange.
mod ownership;
mod population;
mod tree;
mod wire;

pub(in crate::table) use ownership::Ownership;

use crate::{
    BeforeChangeFieldPreparation, EditError, IndexCheckStatus, InspectError, NativeImage,
    WriteBlocker, check_table, header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

fn vector<T>(
    count: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<T>, EditError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(values)
}

/// Reconstruct INDEX bytes into the proof's original-derived expected image.
/// The preparation must be the proof's fresh original-request evaluation, not
/// staging's cached report. No patch list, encoded key or page list is accepted.
pub(in crate::table) fn reconstruct(
    image: &NativeImage,
    table: &str,
    prepared: &BeforeChangeFieldPreparation<'_>,
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let _span = tracing::debug_span!("native.before_change.index_proof").entered();
    check(budget, cancelled)?;
    if prepared.source_bytes().as_ptr() != image.as_bytes().as_ptr()
        || prepared.source_bytes().len() != image.as_bytes().len()
        || expected.len() != image.as_bytes().len()
        || prepared.rejection().is_some()
    {
        return Err(EditError::Preservation);
    }
    let checked = check_table(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    let schema = checked.schema();
    if schema.object_id() != prepared.table_object_id() {
        return Err(EditError::Preservation);
    }
    if checked.indexes().is_empty() {
        // Composition must skip this operation for the old unindexed profile,
        // preserving its existing exact pools and cancellation precedence.
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    if checked
        .indexes()
        .iter()
        .any(|index| index.status() != IndexCheckStatus::Checked)
    {
        return Err(EditError::Unsupported(WriteBlocker::ConstraintCoverage));
    }
    let ownership = Ownership::capture(image, &checked, budget, cancelled)?;
    let population = population::capture(image, schema, prepared, budget, cancelled)?;
    for (position, checked_index) in checked.indexes().iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, position as u64)
            .map_err(InspectError::from)?;
        let index = checked_index.index();
        if checked.indexes()[..position]
            .iter()
            .any(|prior| prior.index().data_number() == index.data_number())
        {
            continue;
        }
        let tree = ownership.require(image, schema, index, budget, cancelled)?;
        wire::reconstruct(
            &wire::Operation {
                image,
                checked: &checked,
                index,
                population: &population,
                tree,
            },
            expected,
            budget,
            cancelled,
        )?;
    }
    check(budget, cancelled)?;
    Ok(())
}
