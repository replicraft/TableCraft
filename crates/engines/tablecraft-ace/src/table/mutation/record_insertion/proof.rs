//! Independent complete ordinary-record reconstruction below primary policy.
pub(in crate::table::mutation) mod fixed;
pub(in crate::table::mutation) mod layout;
pub(in crate::table::mutation) mod payload;

use crate::{
    EditError, InspectError, TableSchema, WriteBlocker,
    format::{
        data_type::{BOOLEAN, LONG_TEXT, OLE},
        page::PAGE_BYTES,
        reader::Reader,
        row::{
            COLUMN_COUNT_BYTES, MASK_BITS, SLOT_BYTES, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES,
            slot_offset,
        },
    },
    table::{record, record_layout},
};
use std::io::Cursor;
use tablecraft_core::values::Value;

pub(in crate::table::mutation) fn assemble(
    schema: &TableSchema,
    shape: &layout::Shape,
    values: &[Value],
    payloads: Vec<Vec<u8>>,
    end: usize,
    resolved_length: Option<usize>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<u8>, EditError> {
    let variable_count = shape.variables.len();
    let footer = if variable_count == 0 {
        0
    } else {
        VARIABLE_OFFSET_BYTES
            .checked_mul(variable_count + 1)
            .and_then(|bytes| bytes.checked_add(VARIABLE_COUNT_BYTES))
            .ok_or(InspectError::StructureLimit)?
    };
    let length = end
        .checked_add(footer)
        .and_then(|bytes| bytes.checked_add(shape.mask_bytes))
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
    let count =
        u16::try_from(schema.layout.max_columns).map_err(|_| InspectError::StructureLimit)?;
    bytes[..COLUMN_COUNT_BYTES].copy_from_slice(&count.to_le_bytes());
    let mask = length - shape.mask_bytes;
    for (position, column) in schema.columns().iter().enumerate() {
        reader.tick()?;
        let present = if column.fixed() {
            let start = if column.native_type() == BOOLEAN {
                0
            } else {
                COLUMN_COUNT_BYTES
                    .checked_add(column.field.offset)
                    .ok_or(InspectError::StructureLimit)?
            };
            let stop = start
                .checked_add(column.storage_length())
                .ok_or(InspectError::StructureLimit)?;
            let target = bytes
                .get_mut(start..stop)
                .filter(|_| stop <= shape.fixed_end)
                .ok_or(EditError::Preservation)?;
            fixed::write(column, &values[position], target)?
        } else {
            !matches!(values[position], Value::Null)
        };
        if present {
            let number = column.number();
            bytes[mask + number / MASK_BITS] |= 1 << (number % MASK_BITS);
        }
    }

    if variable_count != 0 {
        let count_at = mask - VARIABLE_COUNT_BYTES;
        let count = u16::try_from(variable_count).map_err(|_| InspectError::StructureLimit)?;
        bytes[count_at..mask].copy_from_slice(&count.to_le_bytes());
        let mut start = shape.fixed_end;
        for (ordinal, payload) in payloads.iter().enumerate() {
            put_offset(&mut bytes, count_at, ordinal, start)?;
            let stop = start
                .checked_add(payload.len())
                .ok_or(InspectError::StructureLimit)?;
            reader.work(payload.len())?;
            bytes[start..stop].copy_from_slice(payload);
            start = stop;
        }
        put_offset(&mut bytes, count_at, variable_count, start)?;
        if start != end {
            return Err(EditError::Preservation);
        }
    }

    record_layout::parse(&bytes, schema, reader.budget)?;
    if let Some(resolved_length) = resolved_length {
        if resolved_length != length {
            return Err(EditError::Preservation);
        }
        let mut selected = Vec::new();
        reserve(&mut selected, schema.columns().len(), reader)?;
        for (position, field) in schema.columns().iter().enumerate() {
            reader.tick()?;
            if !matches!(field.native_type(), LONG_TEXT | OLE) {
                selected.push(position);
            }
        }
        let decoded = record::decode_selected(&bytes, schema, Some(&selected), reader)?;
        if decoded.len() != selected.len() {
            return Err(EditError::Preservation);
        }
        for (value, position) in decoded.iter().zip(selected) {
            reader.tick()?;
            if value != &values[position] {
                return Err(EditError::Preservation);
            }
        }
    } else if record::decode(&bytes, schema, reader)? != values {
        return Err(EditError::Preservation);
    }
    reader.tick()?;
    Ok(bytes)
}

fn put_offset(
    bytes: &mut [u8],
    count: usize,
    ordinal: usize,
    value: usize,
) -> Result<(), EditError> {
    let offset = count
        .checked_sub((ordinal + 1) * VARIABLE_OFFSET_BYTES)
        .ok_or(EditError::Preservation)?;
    let value = u16::try_from(value).map_err(|_| InspectError::StructureLimit)?;
    bytes[offset..offset + VARIABLE_OFFSET_BYTES].copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn reserve<T>(
    output: &mut Vec<T>,
    count: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.tick()?;
    reader
        .budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    output
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    Ok(())
}

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub(in crate::table::mutation) mod current;
