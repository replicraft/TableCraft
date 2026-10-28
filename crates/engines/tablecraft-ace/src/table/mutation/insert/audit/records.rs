//! Reconstruct new ordinary records from original field geometry and values.
use super::{Row, reserve};
use crate::{
    EditError, InspectError, TableSchema, WriteBlocker,
    format::{
        data_type::{INT32, MAX_SHORT_BINARY_BYTES, SHORT_BINARY},
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        page::PAGE_BYTES,
        reader::Reader,
        row::{
            COLUMN_COUNT_BYTES, MASK_BITS, SLOT_BYTES, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES,
            slot_offset,
        },
    },
    header::check,
    table::{mutation::insert::RowInsert, record_layout},
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;

const COLUMN_COUNT: usize = 2;
const VARIABLE_COUNT: usize = 1;

pub(super) fn qualify(
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    check(budget, cancelled)?;
    if schema.columns().len() != COLUMN_COUNT
        || schema.layout.max_columns != COLUMN_COUNT
        || schema.layout.variable_slots != VARIABLE_COUNT
    {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let id = match (
        schema.columns()[0].native_type(),
        schema.columns()[1].native_type(),
    ) {
        (INT32, SHORT_BINARY) => 0,
        (SHORT_BINARY, INT32) => 1,
        _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
    };
    let fixed = &schema.columns()[id];
    let binary = &schema.columns()[1 - id];
    for (position, field) in schema.columns().iter().enumerate() {
        check(budget, cancelled)?;
        if field.number() != position {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
    }

    if !fixed.fixed()
        || fixed.scalar_type() != Some(ScalarType::Int32)
        || fixed.storage_length() != size_of::<i32>()
        || fixed.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
        || fixed.extended_flags() != 0
        || fixed.complex_type_id().is_some()
        || binary.fixed()
        || binary.flags() != UPDATABLE_FLAG
        || binary.extended_flags() != 0
        || binary.complex_type_id().is_some()
        || binary.storage_length() == 0
        || binary.storage_length() > MAX_SHORT_BINARY_BYTES
    {
        return Err(EditError::Unsupported(WriteBlocker::Column));
    }
    let fixed_end = COLUMN_COUNT_BYTES
        .checked_add(fixed.field.offset)
        .and_then(|offset| offset.checked_add(size_of::<i32>()))
        .ok_or(InspectError::StructureLimit)?;
    if binary.field.variable != 0 || schema.layout.fixed_end != fixed_end {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }

    Ok(id)
}

pub(super) fn derive(
    schema: &TableSchema,
    id: usize,
    requests: &[RowInsert],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<Row>, EditError> {
    let mut rows = Vec::new();
    reserve(&mut rows, requests.len(), reader)?;
    for request in requests {
        reader.tick()?;
        reader
            .budget
            .charge(tablecraft_core::limits::Resource::Rows, 1)
            .map_err(InspectError::from)?;
        if request.values().len() != COLUMN_COUNT {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        let value = match request.values()[id] {
            Value::Int32(value) => value,
            _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
        };
        let (payload, present) = match &request.values()[1 - id] {
            Value::Null => (&[][..], false),
            Value::Binary(bytes) if bytes.len() <= schema.columns()[1 - id].storage_length() => {
                (bytes.as_slice(), true)
            }
            _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
        };
        let start = schema.layout.fixed_end;
        let end = start
            .checked_add(payload.len())
            .ok_or(InspectError::StructureLimit)?;
        let mask_bytes = schema.layout.max_columns.div_ceil(MASK_BITS);
        let footer = VARIABLE_OFFSET_BYTES
            .checked_mul(VARIABLE_COUNT + 1)
            .and_then(|length| length.checked_add(VARIABLE_COUNT_BYTES))
            .and_then(|length| length.checked_add(mask_bytes))
            .ok_or(InspectError::StructureLimit)?;
        let length = end
            .checked_add(footer)
            .ok_or(InspectError::StructureLimit)?;
        if length
            .checked_add(SLOT_BYTES)
            .is_none_or(|needed| needed > PAGE_BYTES - slot_offset(0))
        {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }

        let mut bytes = Vec::new();
        reserve(&mut bytes, length, reader)?;
        reader.work(length)?;
        bytes.resize(length, 0);
        bytes[..COLUMN_COUNT_BYTES].copy_from_slice(&(COLUMN_COUNT as u16).to_le_bytes());
        let fixed_start = COLUMN_COUNT_BYTES + schema.columns()[id].field.offset;
        bytes[fixed_start..fixed_start + size_of::<i32>()].copy_from_slice(&value.to_le_bytes());
        bytes[start..end].copy_from_slice(payload);

        let mask = length - mask_bytes;
        let count = mask - VARIABLE_COUNT_BYTES;
        let first_offset = count - VARIABLE_OFFSET_BYTES;
        let end_offset = first_offset - VARIABLE_OFFSET_BYTES;
        let start = u16::try_from(start).map_err(|_| InspectError::StructureLimit)?;
        let end = u16::try_from(end).map_err(|_| InspectError::StructureLimit)?;
        bytes[first_offset..count].copy_from_slice(&start.to_le_bytes());
        bytes[end_offset..first_offset].copy_from_slice(&end.to_le_bytes());
        bytes[count..mask].copy_from_slice(&(VARIABLE_COUNT as u16).to_le_bytes());
        let fixed_number = schema.columns()[id].number();
        bytes[mask + fixed_number / MASK_BITS] |= 1 << (fixed_number % MASK_BITS);
        if present {
            let number = schema.columns()[1 - id].number();
            bytes[mask + number / MASK_BITS] |= 1 << (number % MASK_BITS);
        }
        // The reader is a secondary structural check, never the requested-wire
        // constructor or a source of lengths supplied by the builder.
        record_layout::parse(&bytes, schema, reader.budget)?;
        rows.push(Row {
            value,
            bytes,
            reference: 0,
            range: 0..0,
        });
    }

    Ok(rows)
}
