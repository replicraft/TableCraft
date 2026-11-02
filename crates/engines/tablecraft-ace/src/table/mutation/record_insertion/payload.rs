//! INSERT variable carriers; existing-slot update eligibility remains separate.
use crate::table::mutation::record_serialization::long_value as long_encode;
use crate::{
    EditError, InspectError, NativeColumn, WriteBlocker,
    format::{data_type::NUMERIC_BYTES, layout::UPDATABLE_FLAG},
    header::check,
    table::{
        decimal, long_value::profile as long_profile, mutation::record_serialization::storage,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

pub(in crate::table::mutation) fn qualify(column: &NativeColumn) -> Result<(), EditError> {
    if long_profile::ordinary(column) {
        return Ok(());
    }
    if column.scalar_type() != Some(ScalarType::Decimal) {
        return storage::qualify(column);
    }
    if column.fixed()
        || column.storage_length() != NUMERIC_BYTES
        || column.flags() != UPDATABLE_FLAG
        || column.extended_flags() != 0
        || column.complex_type_id().is_some()
    {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    Ok(())
}

pub(in crate::table::mutation) fn encode(
    column: &NativeColumn,
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    check(budget, cancelled)?;
    qualify(column)?;
    if long_profile::ordinary(column) {
        return long_encode::encode_inline(column, value, budget, cancelled);
    }
    if column.scalar_type() != Some(ScalarType::Decimal) {
        return storage::encode_preferred(column, value, budget, cancelled);
    }
    let value = match value {
        Value::Null => return Ok(Vec::new()),
        Value::Decimal(value) => *value,
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    };
    let (precision, scale) = column
        .decimal_parameters()
        .ok_or(EditError::Unsupported(WriteBlocker::Column))?;
    if value.scale() != scale
        || value.coefficient().unsigned_abs() >= 10_u128.pow(u32::from(precision))
    {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }

    budget
        .charge_allocation(NUMERIC_BYTES as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, NUMERIC_BYTES as u64)
        .map_err(InspectError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(NUMERIC_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    bytes.extend_from_slice(&decimal::encode(value));
    check(budget, cancelled)?;
    Ok(bytes)
}
