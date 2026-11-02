//! Normal Increment insertion retains original presence in the existing TCNA2.
mod binding;
pub(super) mod mode;
pub(super) mod proof;

use crate::{
    EditError, InspectError, NativeEditPlan, NativeImage, OmittedRowInsert,
    header::check,
    limits::MAX_LIVE_ROWS,
    table::mutation::{autonumber_counter, preserved_source::Source, record_insertion::properties},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn plan(
    image: NativeImage,
    source: Source,
    table: &str,
    requests: &[OmittedRowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, EditError> {
    let counter = mode::capture(&image, &source, budget, cancelled)?;

    source
        .records
        .len()
        .checked_add(requests.len())
        .filter(|count| *count <= MAX_LIVE_ROWS)
        .ok_or(InspectError::StructureLimit)?;

    binding::validate(
        source.table.schema(),
        counter.column,
        requests,
        budget,
        cancelled,
    )?;

    let generated = autonumber_counter::prepare(counter, requests.len(), budget, cancelled)?;
    let counter = generated.source;
    let rows = binding::expand(&generated, requests, budget, cancelled)?;

    properties::require_text_declarations(&image, source.table.schema(), &rows, budget, cancelled)?;
    let (facts, optional) =
        super::indexed::qualify(&image, &source, table, &rows, budget, cancelled)?;
    if optional {
        return Err(EditError::Preservation);
    }

    let encoded = super::records::encode_incrementing(
        source.table.schema(),
        &rows,
        counter.column,
        budget,
        cancelled,
    )?;
    let mut after =
        super::indexed::stage(&image, &source, &rows, &encoded, &facts, budget, cancelled)?;
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, (2 * size_of::<i32>()) as u64)
        .map_err(InspectError::from)?;
    let destination = after
        .get_mut(counter.offset..counter.offset + size_of::<i32>())
        .ok_or(EditError::Preservation)?;
    if destination != counter.before.as_slice() {
        return Err(EditError::Preservation);
    }
    destination.copy_from_slice(&generated.after);
    drop(facts);

    let scope = image.catalog().catalog().scope();
    let plan = NativeEditPlan {
        before: image.bytes,
        after,
        changed_rows: requests.len(),
    };
    proof::prove(&plan, scope, table, requests, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(plan)
}

pub(super) fn vector<T>(
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

#[cfg(test)]
mod tests;

#[cfg(test)]
mod companion_original_sources;

#[cfg(test)]
mod companion_sources;

#[cfg(test)]
mod companion_after_sources;

#[cfg(test)]
mod companion_tests;
