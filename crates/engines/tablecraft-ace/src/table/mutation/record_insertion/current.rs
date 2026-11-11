//! Fresh current records retain native logical and variable high-water positions.
use super::{fixed, payload};
use crate::{
    EditError, InspectError, TableSchema, WriteBlocker,
    format::{
        page::PAGE_BYTES,
        row::{
            COLUMN_COUNT_BYTES, MASK_BITS, SLOT_BYTES, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES,
            slot_offset,
        },
    },
    header::check,
    table::record_layout,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

/// An unassigned retained slot is deleted storage, distinct from an omitted
/// caller field or an explicitly supplied NULL in a live descriptor position.
pub(in crate::table::mutation) fn variables(
    schema: &TableSchema,
    generated: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Option<usize>>, EditError> {
    variables_for(schema, Some(generated), budget, cancelled)
}

/// Fresh ordinary retained records have no generated position. All live fields
/// keep their actual native slots; retired fixed storage is initialized, not reused.
pub(in crate::table::mutation) fn ordinary_variables(
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Option<usize>>, EditError> {
    variables_for(schema, None, budget, cancelled)
}

fn variables_for(
    schema: &TableSchema,
    generated: Option<usize>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Option<usize>>, EditError> {
    check(budget, cancelled)?;
    let layout = schema.physical_layout();
    if schema.columns().is_empty()
        || schema.columns().len() > layout.max_columns
        || layout.variable_slots > layout.max_columns
        || generated.is_some_and(|position| {
            schema
                .columns()
                .get(position)
                .is_none_or(|field| !field.fixed())
        })
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }

    budget
        .charge_allocation(
            layout.variable_slots as u64,
            size_of::<Option<usize>>() as u64,
        )
        .map_err(InspectError::from)?;
    let mut variables = Vec::new();
    variables
        .try_reserve_exact(layout.variable_slots)
        .map_err(|_| InspectError::Allocation)?;
    variables.resize(layout.variable_slots, None);

    let mut fixed_end = COLUMN_COUNT_BYTES;
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if field.number() >= layout.max_columns
            || position != 0 && schema.columns()[position - 1].number() >= field.number()
        {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        if field.fixed() {
            let width = if Some(position) == generated {
                fixed::incrementing_width(field)?
            } else {
                fixed::width(field)?
            };
            if width == 0 {
                continue;
            }
            let start = COLUMN_COUNT_BYTES
                .checked_add(field.physical_field().offset)
                .ok_or(InspectError::StructureLimit)?;
            let end = start
                .checked_add(width)
                .ok_or(InspectError::StructureLimit)?;
            if end > layout.fixed_end {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            for prior in &schema.columns()[..position] {
                check(budget, cancelled)?;
                if !prior.fixed() || prior.storage_length() == 0 {
                    continue;
                }
                let left = COLUMN_COUNT_BYTES
                    .checked_add(prior.physical_field().offset)
                    .ok_or(InspectError::StructureLimit)?;
                let right = left
                    .checked_add(prior.storage_length())
                    .ok_or(InspectError::StructureLimit)?;
                if start < right && left < end {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
            }
            fixed_end = fixed_end.max(end);
        } else {
            payload::qualify(field)?;
            let slot = variables
                .get_mut(field.physical_field().variable)
                .ok_or(EditError::Unsupported(WriteBlocker::RowLayout))?;
            if slot.replace(position).is_some() {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
        }
    }

    if generated.is_some() && fixed_end != layout.fixed_end {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }

    Ok(variables)
}

pub(in crate::table::mutation) fn build(
    schema: &TableSchema,
    variables: &[Option<usize>],
    values: &[Value],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    check(budget, cancelled)?;
    if values.len() != schema.columns().len()
        || variables.len() != schema.physical_layout().variable_slots
    {
        return Err(EditError::Preservation);
    }

    budget
        .charge_allocation(variables.len() as u64, size_of::<Vec<u8>>() as u64)
        .map_err(InspectError::from)?;
    let mut payloads = Vec::new();
    payloads
        .try_reserve_exact(variables.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut end = schema.physical_layout().fixed_end;
    for variable in variables {
        check(budget, cancelled)?;
        let bytes = match variable {
            Some(position) => payload::encode(
                &schema.columns()[*position],
                &values[*position],
                budget,
                cancelled,
            )?,
            None => Vec::new(),
        };
        end = end
            .checked_add(bytes.len())
            .ok_or(InspectError::StructureLimit)?;
        payloads.push(bytes);
    }
    let footer = if variables.is_empty() {
        0
    } else {
        (variables.len() + 1)
            .checked_mul(VARIABLE_OFFSET_BYTES)
            .and_then(|bytes| bytes.checked_add(VARIABLE_COUNT_BYTES))
            .ok_or(InspectError::StructureLimit)?
    };
    let mask = end
        .checked_add(footer)
        .ok_or(InspectError::StructureLimit)?;
    let length = mask
        .checked_add(schema.physical_layout().max_columns.div_ceil(MASK_BITS))
        .ok_or(InspectError::StructureLimit)?;
    if length
        .checked_add(SLOT_BYTES)
        .is_none_or(|bytes| bytes > PAGE_BYTES - slot_offset(0))
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }

    budget
        .charge_allocation(length as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, length as u64)
        .map_err(InspectError::from)?;
    let mut row = Vec::new();
    row.try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    row.resize(length, 0);
    let count = u16::try_from(schema.physical_layout().max_columns)
        .map_err(|_| InspectError::StructureLimit)?;
    row[..COLUMN_COUNT_BYTES].copy_from_slice(&count.to_le_bytes());
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        let present = if field.fixed() {
            let start = if field.storage_length() == 0 {
                COLUMN_COUNT_BYTES
            } else {
                COLUMN_COUNT_BYTES + field.physical_field().offset
            };
            fixed::write(
                field,
                &values[position],
                &mut row[start..start + field.storage_length()],
            )?
        } else {
            !matches!(values[position], Value::Null)
        };
        if present {
            row[mask + field.number() / MASK_BITS] |= 1 << (field.number() % MASK_BITS);
        }
    }
    if !variables.is_empty() {
        let count_at = mask - VARIABLE_COUNT_BYTES;
        let count = u16::try_from(variables.len()).map_err(|_| InspectError::StructureLimit)?;
        row[count_at..mask].copy_from_slice(&count.to_le_bytes());
        let mut start = schema.physical_layout().fixed_end;
        for (ordinal, payload) in payloads.iter().enumerate() {
            check(budget, cancelled)?;
            let at = count_at - VARIABLE_OFFSET_BYTES * (ordinal + 1);
            let offset = u16::try_from(start).map_err(|_| InspectError::StructureLimit)?;
            row[at..at + VARIABLE_OFFSET_BYTES].copy_from_slice(&offset.to_le_bytes());
            row[start..start + payload.len()].copy_from_slice(payload);
            start += payload.len();
        }
        let at = count_at - VARIABLE_OFFSET_BYTES * (variables.len() + 1);
        row[at..at + VARIABLE_OFFSET_BYTES].copy_from_slice(
            &u16::try_from(start)
                .map_err(|_| InspectError::StructureLimit)?
                .to_le_bytes(),
        );
    }
    record_layout::parse(&row, schema, budget)?;
    Ok(row)
}
