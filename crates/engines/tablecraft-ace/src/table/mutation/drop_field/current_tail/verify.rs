//! Complete ordered survivor reads complement the exact unchanged storage proof.
use super::{Error, source::Source};
use crate::{
    InspectError, check_table,
    format::page::PAGE_BYTES,
    header::check,
    read_table, read_table_projection,
    table::mutation::{catalog_property::retained_snapshot::Snapshot, current_schema::Family},
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn survivors(
    source: &Source,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    check(budget, cancelled)?;
    let original = source.proof.image();
    let observed = &source.evidence.observed;
    let scope = original.catalog().catalog().scope();
    let count = observed
        .schema
        .columns()
        .len()
        .checked_sub(1)
        .ok_or(Error::Source)?;
    budget.charge_allocation(count as u64, size_of::<&str>() as u64)?;
    let mut names = Vec::new();
    names
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    names.extend(
        observed
            .schema
            .columns()
            .iter()
            .take(count)
            .map(|field| field.name()),
    );
    let before = read_table_projection(
        &mut original.reader(),
        scope,
        observed.schema.name(),
        &names,
        budget,
        cancelled,
    )?;
    let replacement = read_table(
        &mut Cursor::new(after),
        scope,
        observed.schema.name(),
        budget,
        cancelled,
    )?;
    if before.rows().len() != observed.schema.declared_rows() as usize
        || replacement.rows().len() != before.rows().len()
        || replacement.columns().len() != names.len()
    {
        return Err(Error::Preservation);
    }
    for (left, right) in before.rows().iter().zip(replacement.rows()) {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, left.len() as u64)?;
        // Equality is representational here: no SQL coercion or display-format
        // conversion may hide a changed survivor carrier, NULL or native order.
        if left != right {
            return Err(Error::Preservation);
        }
    }
    // The shared complement proof already protects every DATA byte, root slot,
    // relocation, ownership map and external INDEX page. Rechecking the live tree
    // under the replacement schema ensures surviving native keys still resolve.
    let checked = check_table(
        &mut Cursor::new(after),
        scope,
        observed.schema.name(),
        budget,
        cancelled,
    )?;
    if checked.rows() != before.rows().len() {
        return Err(Error::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}

pub(super) fn retained_candidate(
    source: &Source,
    after: &[u8],
    snapshot: &Snapshot<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    let before = source.proof.image();
    let observed = &source.evidence.observed;
    if before.as_bytes().len() != after.len() {
        return Err(Error::Preservation);
    }
    super::super::verify::definition_bytes(before, &source.evidence, after, budget, cancelled)?;
    snapshot.verify_placement(after, budget, cancelled)?;
    let base = observed.schema.definition_page() as usize * PAGE_BYTES;
    let allowed = [
        base..base + PAGE_BYTES,
        snapshot.catalog_page_range(),
        snapshot.property_page_range(),
    ];
    budget.charge(
        Resource::Steps,
        (after.len() as u64)
            .checked_mul(allowed.len() as u64)
            .ok_or(InspectError::StructureLimit)?,
    )?;
    for (offset, (original, current)) in before.as_bytes().iter().zip(after).enumerate() {
        if offset.is_multiple_of(PAGE_BYTES) {
            check(budget, cancelled)?;
        }
        if original != current && !allowed.iter().any(|range| range.contains(&offset)) {
            return Err(Error::Preservation);
        }
    }
    let replacement =
        Family::Retained.image(after, before.catalog().catalog().scope(), budget, cancelled)?;
    let current = Family::Retained
        .observe(&replacement, observed.schema.name(), budget, cancelled)
        .map_err(|error| match error {
            crate::table::mutation::current_schema::Error::Inspect(error) => Error::Inspect(error),
            crate::table::mutation::current_schema::Error::Native(error) => Error::Placement(error),
            crate::table::mutation::current_schema::Error::Source => Error::Source,
        })?;
    if current.schema.object_id() != observed.schema.object_id()
        || current.schema.columns().len() + 1 != observed.schema.columns().len()
        || current.schema.declared_rows() != observed.schema.declared_rows()
        || current.properties.raw_bytes() != snapshot.body()
        || current.properties.groups().len() + 1 != observed.properties.groups().len()
    {
        return Err(Error::Preservation);
    }
    for (old, new) in observed
        .schema
        .columns()
        .iter()
        .take(current.schema.columns().len())
        .zip(current.schema.columns())
    {
        check(budget, cancelled)?;
        if old.name() != new.name() || !old.physical_field().same_storage(new.physical_field()) {
            return Err(Error::Preservation);
        }
    }
    Ok(())
}
