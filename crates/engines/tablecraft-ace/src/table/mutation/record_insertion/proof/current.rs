//! Independent fresh-record geometry with explicit empty retired variable slots.
use super::{fixed, layout, payload};
use crate::{
    EditError, InspectError, TableSchema, WriteBlocker,
    format::{
        data_type::INT32,
        layout::{AUTO_NUMBER_FLAG, FIXED_FLAG, UPDATABLE_FLAG},
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

pub(in crate::table::mutation) struct Shape {
    fixed_end: usize,
    variables: Vec<Option<usize>>,
}

pub(in crate::table::mutation) fn capture(
    schema: &TableSchema,
    generated: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Shape, EditError> {
    capture_for(schema, Some(generated), budget, cancelled)
}

/// Independently bind ordinary native positions without a generation exception.
pub(in crate::table::mutation) fn capture_ordinary(
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Shape, EditError> {
    capture_for(schema, None, budget, cancelled)
}

fn capture_for(
    schema: &TableSchema,
    generated: Option<usize>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Shape, EditError> {
    let declaration = schema.physical_layout();
    if schema.columns().is_empty()
        || schema.columns().len() > declaration.max_columns
        || declaration.variable_slots > declaration.max_columns
        || generated.is_some_and(|position| position >= schema.columns().len())
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }

    budget
        .charge_allocation(
            declaration.variable_slots as u64,
            size_of::<Option<usize>>() as u64,
        )
        .map_err(InspectError::from)?;
    let mut variables = Vec::new();
    variables
        .try_reserve_exact(declaration.variable_slots)
        .map_err(|_| InspectError::Allocation)?;
    variables.resize(declaration.variable_slots, None);

    let mut fixed_end = COLUMN_COUNT_BYTES;
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if field.number() >= declaration.max_columns
            || position != 0 && schema.columns()[position - 1].number() >= field.number()
            || field.complex_type_id().is_some()
        {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        if field.fixed() {
            let width = fixed::width(field).ok_or(EditError::Unsupported(WriteBlocker::Column))?;
            let expected_flags = if Some(position) == generated {
                if field.native_type() != INT32 {
                    return Err(EditError::Unsupported(WriteBlocker::Column));
                }
                FIXED_FLAG | UPDATABLE_FLAG | AUTO_NUMBER_FLAG
            } else {
                FIXED_FLAG | UPDATABLE_FLAG
            };
            if field.flags() != expected_flags
                || field.extended_flags() != 0
                || field.storage_length() != width
            {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
            if width == 0 {
                continue;
            }
            let left = COLUMN_COUNT_BYTES
                .checked_add(field.physical_field().offset)
                .ok_or(InspectError::StructureLimit)?;
            let right = left
                .checked_add(width)
                .ok_or(InspectError::StructureLimit)?;
            for prior in &schema.columns()[..position] {
                check(budget, cancelled)?;
                if !prior.fixed() || prior.storage_length() == 0 {
                    continue;
                }
                let start = COLUMN_COUNT_BYTES
                    .checked_add(prior.physical_field().offset)
                    .ok_or(InspectError::StructureLimit)?;
                let stop = start
                    .checked_add(prior.storage_length())
                    .ok_or(InspectError::StructureLimit)?;
                if left < stop && start < right {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
            }
            fixed_end = fixed_end.max(right);
        } else {
            layout::variable(field)?;
            let selected = variables
                .get_mut(field.physical_field().variable)
                .ok_or(EditError::Unsupported(WriteBlocker::RowLayout))?;
            if selected.replace(position).is_some() {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
        }
    }
    if generated.is_some_and(|position| {
        !schema.columns()[position].fixed() || fixed_end != declaration.fixed_end
    }) || fixed_end > declaration.fixed_end
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    // Ordinary sources may retain fixed bytes beyond all live fields. Fresh
    // records retain that exact physical prefix, initialized independently.
    let fixed_end = if generated.is_none() {
        declaration.fixed_end
    } else {
        fixed_end
    };

    Ok(Shape {
        fixed_end,
        variables,
    })
}

pub(in crate::table::mutation) fn reconstruct(
    schema: &TableSchema,
    shape: &Shape,
    values: &[Value],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    if values.len() != schema.columns().len() {
        return Err(EditError::Preservation);
    }

    budget
        .charge_allocation(shape.variables.len() as u64, size_of::<Vec<u8>>() as u64)
        .map_err(InspectError::from)?;
    let mut payloads = Vec::new();
    payloads
        .try_reserve_exact(shape.variables.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut end = shape.fixed_end;
    for selected in &shape.variables {
        check(budget, cancelled)?;
        let encoded = match selected {
            Some(position) => payload::encode(
                &schema.columns()[*position],
                &values[*position],
                budget,
                cancelled,
            )?,
            None => Vec::new(),
        };
        end = end
            .checked_add(encoded.len())
            .ok_or(InspectError::StructureLimit)?;
        payloads.push(encoded);
    }
    let footer = if shape.variables.is_empty() {
        0
    } else {
        (shape.variables.len() + 1)
            .checked_mul(VARIABLE_OFFSET_BYTES)
            .and_then(|bytes| bytes.checked_add(VARIABLE_COUNT_BYTES))
            .ok_or(InspectError::StructureLimit)?
    };
    let mask_bytes = schema.physical_layout().max_columns.div_ceil(MASK_BITS);
    let length = end
        .checked_add(footer)
        .and_then(|bytes| bytes.checked_add(mask_bytes))
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
    let mut expected = Vec::new();
    expected
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    expected.extend(
        u16::try_from(schema.physical_layout().max_columns)
            .map_err(|_| InspectError::StructureLimit)?
            .to_le_bytes(),
    );
    expected.resize(shape.fixed_end, 0);
    budget
        .charge_allocation(mask_bytes as u64, 1)
        .map_err(InspectError::from)?;
    let mut mask = Vec::new();
    mask.try_reserve_exact(mask_bytes)
        .map_err(|_| InspectError::Allocation)?;
    mask.resize(mask_bytes, 0);
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
                &mut expected[start..start + field.storage_length()],
            )?
        } else {
            !matches!(values[position], Value::Null)
        };
        if present {
            mask[field.number() / MASK_BITS] |= 1 << (field.number() % MASK_BITS);
        }
    }
    for encoded in &payloads {
        check(budget, cancelled)?;
        expected.extend_from_slice(encoded);
    }
    if !shape.variables.is_empty() {
        let mut offset = end;
        expected.extend(
            u16::try_from(offset)
                .map_err(|_| InspectError::StructureLimit)?
                .to_le_bytes(),
        );
        for encoded in payloads.iter().rev() {
            check(budget, cancelled)?;
            offset -= encoded.len();
            expected.extend(
                u16::try_from(offset)
                    .map_err(|_| InspectError::StructureLimit)?
                    .to_le_bytes(),
            );
        }
        expected.extend(
            u16::try_from(shape.variables.len())
                .map_err(|_| InspectError::StructureLimit)?
                .to_le_bytes(),
        );
    }
    expected.extend(mask);
    if expected.len() != length {
        return Err(EditError::Preservation);
    }
    record_layout::parse(&expected, schema, budget)?;
    Ok(expected)
}
