//! Existing native Yes/No fields own a value bit and no payload range.
use crate::{
    EditError, TableSchema, WriteBlocker,
    format::{data_type::BOOLEAN, row::MASK_BITS},
    table::{mutation::field, record_layout},
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;

pub(in crate::table::mutation) fn qualify(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    super::check(budget, cancelled)?;
    // The shared fixed binding deliberately ignores Boolean's unused offset.
    field::fixed_column_at(schema, column, ScalarType::Boolean, 0)?;
    let selected = &schema.columns()[column];
    if selected.native_type() != BOOLEAN {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    let record = record_layout::parse(before, schema, budget)?;
    if selected.number() >= record.count {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let mask = record.mask_start + selected.number() / MASK_BITS;
    before.get(mask).ok_or(EditError::Preservation)?;
    Ok(mask)
}

pub(in crate::table::mutation) fn build(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    value: &Value,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let Value::Boolean(value) = value else {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    };
    let mask = qualify(schema, column, before, budget, cancelled)?;
    let mut after = super::copy(before, budget, cancelled)?;
    let bit = 1 << (schema.columns()[column].number() % MASK_BITS);
    after[mask] = (after[mask] & !bit) | if *value { bit } else { 0 };
    Ok(after)
}
