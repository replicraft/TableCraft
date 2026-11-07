//! Explicit fresh Increment declaration; initial values are never generated.
use super::{
    CreateDatabaseError,
    input::{Columns, Rows},
};
use crate::{
    format::layout::{AUTO_NUMBER_FLAG, FIXED_FLAG, UPDATABLE_FLAG},
    header::check,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

pub(super) const LONG_INCREMENT_FLAGS: u8 = FIXED_FLAG | UPDATABLE_FLAG | AUTO_NUMBER_FLAG;
const GENERATED_COLUMN: usize = 0;

pub(super) fn qualify(
    columns: Columns<'_>,
    rows: Rows<'_>,
    selected: Option<usize>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CreateDatabaseError> {
    let Some(selected) = selected else {
        // Preserve the old non-generated request's costs and byte behavior.
        return Ok(());
    };
    check(budget, cancelled)?;
    if selected != GENERATED_COLUMN
        || columns.scalar(selected) != ScalarType::Int32
        || rows.count() != 0
        || columns.primary_key_name(selected).is_none()
    {
        return Err(CreateDatabaseError::Declaration);
    }
    for position in 0..columns.count() {
        check(budget, cancelled)?;
        if position == selected
            && (columns.policies(position) != (None, None)
                || columns.default_literal(position).is_some())
            || position != selected && columns.primary_key_name(position).is_some()
        {
            return Err(CreateDatabaseError::Declaration);
        }
    }
    // The existing definition writer authors LAST0 from its zeroed header.
    // No live-value scan, seed inference, property expression or ID allocation.
    check(budget, cancelled)?;
    Ok(())
}
