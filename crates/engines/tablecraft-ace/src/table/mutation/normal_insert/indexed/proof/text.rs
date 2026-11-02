//! Original/request Text proof. No producer key, patch or reservation enters.
use super::RequestedRows;
use crate::{
    EditError, InspectError, NativeImage, NativeIndex,
    format::page::PAGE_BYTES,
    header::check,
    table::{
        indexes::mutation::normal_index_audit::text::{
            distinct, original_value, requested, require_population, sort, write_population,
        },
        mutation::preserved_source::Source,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn reconstruct(
    image: &NativeImage,
    source: &Source,
    index: &NativeIndex,
    rows: RequestedRows<'_>,
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let RequestedRows {
        requests,
        references,
    } = rows;
    let [key] = index.columns() else {
        return Err(EditError::Preservation);
    };
    if requests.len() != references.len() || source.table.rows().len() != source.records.len() {
        return Err(EditError::Preservation);
    }
    let schema = source.table.schema();
    budget
        .charge(Resource::Steps, schema.columns().len() as u64)
        .map_err(InspectError::from)?;
    let column = schema
        .columns()
        .iter()
        .position(|field| field.number() == key.number())
        .ok_or(EditError::Preservation)?;
    let field = &schema.columns()[column];
    let capacity = source
        .records
        .len()
        .checked_add(requests.len())
        .ok_or(InspectError::StructureLimit)?;
    let mut keys = super::super::vector(capacity, budget, cancelled)?;
    for (record, observed) in source.records.iter().zip(source.table.rows()) {
        check(budget, cancelled)?;
        let value = original_value(
            &record.data,
            schema,
            field,
            observed.get(column).ok_or(EditError::Preservation)?,
            budget,
            cancelled,
        )?;
        keys.push(requested(
            field,
            value,
            record.reference,
            key.descending(),
            budget,
            cancelled,
        )?);
    }
    sort(&mut keys, budget, cancelled)?;
    let base = (index.root_page() as usize)
        .checked_mul(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let end = base
        .checked_add(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    require_population(
        image
            .as_bytes()
            .get(base..end)
            .ok_or(EditError::Preservation)?,
        &keys,
        budget,
        cancelled,
    )?;
    let before_distinct = distinct(&keys, budget, cancelled)?;
    for (request, reference) in requests.iter().zip(references) {
        check(budget, cancelled)?;
        keys.push(requested(
            field,
            request
                .values()
                .get(column)
                .ok_or(EditError::Preservation)?,
            *reference,
            key.descending(),
            budget,
            cancelled,
        )?);
    }
    sort(&mut keys, budget, cancelled)?;
    let after_distinct = distinct(&keys, budget, cancelled)?;
    write_population(
        expected.get_mut(base..end).ok_or(EditError::Preservation)?,
        &keys,
        budget,
        cancelled,
    )?;
    super::reconstruct_history(
        image,
        source,
        index,
        (before_distinct, after_distinct),
        expected,
        budget,
        cancelled,
    )
}

#[cfg(test)]
mod tests;
