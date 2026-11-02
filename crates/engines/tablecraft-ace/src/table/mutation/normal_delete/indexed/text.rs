//! Independent decreasing Text population, preserving historical counters.
use super::vector;
use crate::{
    EditError, InspectError, NativeImage, NativeIndex,
    format::page::PAGE_BYTES,
    header::check,
    table::{
        indexes::mutation::normal_index_audit::text::{
            original_value, requested, require_population, sort, write_population,
        },
        mutation::preserved_source::Source,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn reconstruct(
    image: &NativeImage,
    source: &Source,
    index: &NativeIndex,
    removed: &[u32],
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let [key] = index.columns() else {
        return Err(EditError::Preservation);
    };
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
    if source.records.len() != source.table.rows().len() {
        return Err(EditError::Preservation);
    }
    let mut original = vector(source.records.len(), budget, cancelled)?;
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
        original.push(requested(
            field,
            value,
            record.reference,
            key.descending(),
            budget,
            cancelled,
        )?);
    }
    sort(&mut original, budget, cancelled)?;
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
        &original,
        budget,
        cancelled,
    )?;
    let mut survivors = vector(original.len(), budget, cancelled)?;
    let mut count = 0;
    for wire in original {
        check(budget, cancelled)?;
        let reference = u32::from_be_bytes(
            wire.get(
                wire.len()
                    .checked_sub(size_of::<u32>())
                    .ok_or(EditError::Preservation)?..,
            )
            .ok_or(EditError::Preservation)?
            .try_into()
            .map_err(|_| EditError::Preservation)?,
        );
        budget
            .charge(Resource::Steps, removed.len() as u64)
            .map_err(InspectError::from)?;
        if removed.contains(&reference) {
            count += 1;
        } else {
            survivors.push(wire);
        }
    }
    if count != removed.len() {
        return Err(EditError::Preservation);
    }
    if removed.is_empty() {
        return Ok(());
    }
    write_population(
        expected.get_mut(base..end).ok_or(EditError::Preservation)?,
        &survivors,
        budget,
        cancelled,
    )
}

#[cfg(test)]
mod tests;
