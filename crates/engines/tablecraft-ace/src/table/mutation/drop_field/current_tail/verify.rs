//! Complete ordered survivor reads complement the exact unchanged storage proof.
use super::{Error, source::Source};
use crate::{InspectError, check_table, header::check, read_table, read_table_projection};
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
