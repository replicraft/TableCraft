//! Independent append-based proof of a single existing Boolean value bit.
use crate::{
    EditError, InspectError, TableSchema,
    format::{
        data_type::BOOLEAN,
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        row::MASK_BITS,
    },
    header::check,
    table::record_layout,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

pub(in crate::table::mutation) fn reconstruct(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    check(budget, cancelled)?;
    let selected = schema
        .columns()
        .get(column)
        .ok_or(EditError::Preservation)?;
    if selected.native_type() != BOOLEAN
        || selected.scalar_type() != Some(ScalarType::Boolean)
        || !selected.fixed()
        || selected.storage_length() != 0
        || selected.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
        || selected.extended_flags() != 0
    {
        return Err(EditError::Preservation);
    }
    let Value::Boolean(value) = value else {
        return Err(EditError::Preservation);
    };
    let original = record_layout::parse(before, schema, budget)?;
    if selected.number() >= original.count {
        return Err(EditError::Preservation);
    }
    let at = original.mask_start + selected.number() / MASK_BITS;
    let mask = *before.get(at).ok_or(EditError::Preservation)?;
    let bit = 1 << (selected.number() % MASK_BITS);
    let changed = if *value { mask | bit } else { mask & !bit };
    budget
        .charge_allocation(before.len() as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, before.len() as u64)
        .map_err(InspectError::from)?;
    let mut expected = Vec::new();
    expected
        .try_reserve_exact(before.len())
        .map_err(|_| InspectError::Allocation)?;
    expected.extend_from_slice(&before[..at]);
    expected.push(changed);
    expected.extend_from_slice(&before[at + 1..]);
    check(budget, cancelled)?;
    Ok(expected)
}
