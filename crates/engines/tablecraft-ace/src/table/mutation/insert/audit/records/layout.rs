//! Independent declaration and physical-span qualification for fresh records.
use super::fixed;
use crate::{
    EditError, InspectError, NativeColumn, TableSchema, WriteBlocker,
    format::{
        data_type::{
            LONG_TEXT, MAX_SHORT_BINARY_BYTES, MAX_SHORT_TEXT_BYTES, NUMERIC, NUMERIC_BYTES, OLE,
            SHORT_BINARY, SHORT_TEXT,
        },
        layout::{FIXED_FLAG, UNICODE_COMPRESSION_FLAG, UPDATABLE_FLAG},
        row::{COLUMN_COUNT_BYTES, MASK_BITS},
    },
    header::check,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

pub(super) struct Shape {
    pub fixed_end: usize,
    pub mask_bytes: usize,
    pub variables: Vec<usize>,
}

pub(super) fn capture(
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Shape, EditError> {
    check(budget, cancelled)?;
    let count = schema.columns().len();
    if count == 0 || schema.layout.max_columns != count {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }

    let variables = schema.layout.variable_slots;
    budget
        .charge_allocation(variables as u64, size_of::<Option<usize>>() as u64)
        .map_err(InspectError::from)?;
    let mut ordinals = Vec::new();
    ordinals
        .try_reserve_exact(variables)
        .map_err(|_| InspectError::Allocation)?;
    ordinals.resize(variables, None);

    let mut fixed_end = COLUMN_COUNT_BYTES;
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if field.number() != position || field.complex_type_id().is_some() {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        if field.fixed() {
            let width = fixed::width(field).ok_or(EditError::Unsupported(WriteBlocker::Column))?;
            if field.storage_length() != width
                || field.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
                || field.extended_flags() != 0
            {
                return Err(EditError::Unsupported(WriteBlocker::Column));
            }
            // Boolean's stored offset does not own any payload bytes. The
            // final extent check also prevents it from inventing padding.
            if width == 0 {
                continue;
            }
            let start = COLUMN_COUNT_BYTES
                .checked_add(field.field.offset)
                .ok_or(InspectError::StructureLimit)?;
            let end = start
                .checked_add(width)
                .ok_or(InspectError::StructureLimit)?;
            fixed_end = fixed_end.max(end);
            for prior in &schema.columns()[..position] {
                check(budget, cancelled)?;
                if !prior.fixed() || prior.storage_length() == 0 || width == 0 {
                    continue;
                }
                let prior_start = COLUMN_COUNT_BYTES
                    .checked_add(prior.field.offset)
                    .ok_or(InspectError::StructureLimit)?;
                let prior_end = prior_start
                    .checked_add(prior.storage_length())
                    .ok_or(InspectError::StructureLimit)?;
                if start < prior_end && prior_start < end {
                    return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                }
            }
        } else {
            variable(field)?;
            let ordinal = ordinals
                .get_mut(field.field.variable)
                .ok_or(EditError::Unsupported(WriteBlocker::RowLayout))?;
            if ordinal.replace(position).is_some() {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
        }
    }

    if fixed_end != schema.layout.fixed_end || ordinals.iter().any(Option::is_none) {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }

    budget
        .charge_allocation(variables as u64, size_of::<usize>() as u64)
        .map_err(InspectError::from)?;
    let mut ordered = Vec::new();
    ordered
        .try_reserve_exact(variables)
        .map_err(|_| InspectError::Allocation)?;
    for ordinal in ordinals {
        check(budget, cancelled)?;
        ordered.push(ordinal.ok_or(EditError::Preservation)?);
    }

    Ok(Shape {
        fixed_end,
        mask_bytes: count.div_ceil(MASK_BITS),
        variables: ordered,
    })
}

fn variable(field: &NativeColumn) -> Result<(), EditError> {
    if matches!(field.native_type(), LONG_TEXT | OLE) {
        let valid = field.flags() == UPDATABLE_FLAG
            && field.storage_length() == 0
            && (field.extended_flags() == 0
                || (field.native_type() == LONG_TEXT
                    && field.extended_flags() == UNICODE_COMPRESSION_FLAG));
        return if valid {
            Ok(())
        } else {
            Err(EditError::Unsupported(WriteBlocker::Column))
        };
    }
    let valid = field.flags() == UPDATABLE_FLAG
        && field.storage_length() > 0
        && match (field.native_type(), field.scalar_type()) {
            (SHORT_TEXT, Some(ScalarType::Text)) => {
                field.storage_length() <= MAX_SHORT_TEXT_BYTES
                    && field.storage_length().is_multiple_of(size_of::<u16>())
                    && field.extended_flags() & !UNICODE_COMPRESSION_FLAG == 0
            }
            (NUMERIC, Some(ScalarType::Decimal)) => {
                field.storage_length() == NUMERIC_BYTES && field.extended_flags() == 0
            }
            (SHORT_BINARY, Some(ScalarType::Binary)) => {
                field.storage_length() <= MAX_SHORT_BINARY_BYTES && field.extended_flags() == 0
            }
            _ => false,
        };
    if !valid {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    Ok(())
}
