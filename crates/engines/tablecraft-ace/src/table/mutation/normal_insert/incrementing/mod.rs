//! Normal Increment insertion retains original presence in the existing TCNA2.
pub(super) mod mode;
pub(super) mod proof;

use crate::{
    EditError, InsertFieldValue, InspectError, NativeEditPlan, NativeImage, OmittedRowInsert,
    RowInsert, WriteBlocker,
    header::check,
    limits::MAX_LIVE_ROWS,
    table::mutation::{autonumber_counter, preserved_source::Source},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

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
    validate_requests(requests, budget, cancelled)?;
    let generated = autonumber_counter::prepare(counter, requests.len(), budget, cancelled)?;
    let counter = generated.source;
    let mut rows = vector(requests.len(), budget, cancelled)?;
    for (request, value) in requests.iter().zip(&generated.values) {
        check(budget, cancelled)?;
        let InsertFieldValue::Supplied(Value::Int32(score)) = &request.values()[1] else {
            return Err(EditError::Preservation);
        };
        let mut values = vector(2, budget, cancelled)?;
        values.push(Value::Int32(*value));
        values.push(Value::Int32(*score));
        rows.push(RowInsert::new(values));
    }
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

fn validate_requests(
    requests: &[OmittedRowInsert],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    for request in requests {
        check(budget, cancelled)?;
        if !matches!(
            request.values(),
            [
                InsertFieldValue::Omitted,
                InsertFieldValue::Supplied(Value::Int32(_))
            ]
        ) {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
    }
    check(budget, cancelled)?;
    Ok(())
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
