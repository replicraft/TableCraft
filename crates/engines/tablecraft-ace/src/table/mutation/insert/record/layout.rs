//! Complete declared field coverage, fixed-span exclusivity and variable order.
use super::{field, payload};
use crate::{
    EditError, InspectError, TableSchema, WriteBlocker, format::row::COLUMN_COUNT_BYTES,
    header::check,
};
use tablecraft_core::limits::ResourceBudget;

const UNASSIGNED_FIELD: usize = usize::MAX;

pub(super) fn variables(
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<usize>, EditError> {
    check(budget, cancelled)?;
    let fields = schema.columns();
    let count = fields.len();
    if count == 0
        || count != schema.layout.max_columns
        || schema.layout.variable_slots > count
        || schema.layout.fixed_end < COLUMN_COUNT_BYTES
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    budget
        .charge_allocation(
            schema.layout.variable_slots as u64,
            size_of::<usize>() as u64,
        )
        .map_err(InspectError::from)?;
    let mut variables = Vec::new();
    variables
        .try_reserve_exact(schema.layout.variable_slots)
        .map_err(|_| InspectError::Allocation)?;
    variables.resize(schema.layout.variable_slots, UNASSIGNED_FIELD);

    let mut fixed_end = COLUMN_COUNT_BYTES;
    for (position, column) in fields.iter().enumerate() {
        check(budget, cancelled)?;
        if column.number() != position {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        if column.fixed() {
            let width = field::width(column)?;
            // Boolean's bit has no payload range. Its unused offset cannot
            // invent a fixed tail beyond the actual nonzero payload extent.
            if width == 0 {
                continue;
            }
            let start = COLUMN_COUNT_BYTES
                .checked_add(column.field.offset)
                .ok_or(InspectError::StructureLimit)?;
            let end = start
                .checked_add(width)
                .ok_or(InspectError::StructureLimit)?;
            if end > schema.layout.fixed_end {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            fixed_end = fixed_end.max(end);
            for previous in &fields[..position] {
                check(budget, cancelled)?;
                if !previous.fixed() || previous.storage_length() == 0 {
                    continue;
                }
                let other_start = COLUMN_COUNT_BYTES
                    .checked_add(previous.field.offset)
                    .ok_or(InspectError::StructureLimit)?;
                let other_end = other_start
                    .checked_add(previous.storage_length())
                    .ok_or(InspectError::StructureLimit)?;
                if start < other_end && other_start < end {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
            }
        } else {
            payload::qualify(column)?;
            if column.storage_length() == 0 || column.complex_type_id().is_some() {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
            let selected = variables
                .get_mut(column.field.variable)
                .ok_or(EditError::Unsupported(WriteBlocker::RowLayout))?;
            if *selected != UNASSIGNED_FIELD {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            *selected = position;
        }
    }
    if fixed_end != schema.layout.fixed_end || variables.contains(&UNASSIGNED_FIELD) {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    Ok(variables)
}
