//! Reconstruct complete new rows independently from schema and explicit values.
mod fixed;
pub(super) mod layout;
pub(super) mod payload;
#[cfg(test)]
mod tests;

use super::{Row, reserve};
use crate::{
    EditError, InspectError, TableSchema, WriteBlocker,
    format::{
        data_type::{BOOLEAN, INT32, LONG_TEXT, OLE},
        page::PAGE_BYTES,
        reader::Reader,
        row::{
            COLUMN_COUNT_BYTES, MASK_BITS, SLOT_BYTES, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES,
            slot_offset,
        },
    },
    table::{mutation::insert::RowInsert, record, record_layout},
};
use std::io::Cursor;
use tablecraft_core::{limits::ResourceBudget, values::Value};

pub(super) fn qualify(
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    layout::capture(schema, budget, cancelled)?;
    Ok(())
}

pub(super) fn derive(
    schema: &TableSchema,
    id: usize,
    requests: &[RowInsert],
    resolved: Option<&[super::long_values::ResolvedRow]>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<Row>, EditError> {
    derive_with_generated(schema, id, requests, resolved, None, reader)
}

pub(super) fn derive_incrementing(
    schema: &TableSchema,
    id: usize,
    requests: &[RowInsert],
    resolved: Option<&[super::long_values::ResolvedRow]>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<Row>, EditError> {
    derive_with_generated(schema, id, requests, resolved, Some(id), reader)
}

fn derive_with_generated(
    schema: &TableSchema,
    id: usize,
    requests: &[RowInsert],
    resolved: Option<&[super::long_values::ResolvedRow]>,
    generated: Option<usize>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<Row>, EditError> {
    let shape = if let Some(column) = generated {
        layout::capture_incrementing(schema, column, reader.budget, reader.cancelled)?
    } else {
        layout::capture(schema, reader.budget, reader.cancelled)?
    };

    let primary = schema.columns().get(id).ok_or(EditError::Preservation)?;
    if primary.native_type() != INT32 || !primary.fixed() {
        return Err(EditError::Preservation);
    }

    let mut rows = Vec::new();
    reserve(&mut rows, requests.len(), reader)?;
    if resolved.is_some_and(|rows| rows.len() != requests.len()) {
        return Err(EditError::Preservation);
    }
    for (row_number, request) in requests.iter().enumerate() {
        reader.tick()?;
        reader
            .budget
            .charge(tablecraft_core::limits::Resource::Rows, 1)
            .map_err(InspectError::from)?;
        if request.values().len() != schema.columns().len() {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        if !matches!(request.values()[id], Value::Int32(_)) {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }

        let mut payloads = Vec::new();
        reserve(&mut payloads, shape.variables.len(), reader)?;
        let mut end = shape.fixed_end;
        for position in &shape.variables {
            reader.tick()?;
            let field = &schema.columns()[*position];
            let bytes = if matches!(field.native_type(), LONG_TEXT | OLE)
                && let Some(resolved) = resolved
            {
                let row = &resolved[row_number];
                reader.work(row.fields.len())?;
                let mut matching = row.fields.iter().filter(|field| field.column == *position);
                let descriptor = matching.next().ok_or(EditError::Preservation)?;
                if matching.next().is_some() {
                    return Err(EditError::Preservation);
                }
                super::copied(&descriptor.descriptor, reader)?
            } else {
                payload::encode(
                    field,
                    &request.values()[*position],
                    reader.budget,
                    reader.cancelled,
                )?
            };
            end = end
                .checked_add(bytes.len())
                .ok_or(InspectError::StructureLimit)?;
            payloads.push(bytes);
        }

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
                fixed::write(column, &request.values()[position], target)?
            } else {
                !matches!(request.values()[position], Value::Null)
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
        if let Some(resolved) = resolved {
            let resolved = &resolved[row_number];
            if resolved.record_length != length {
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
            let values = record::decode_selected(&bytes, schema, Some(&selected), reader)?;
            if values.len() != selected.len() {
                return Err(EditError::Preservation);
            }
            for (value, position) in values.iter().zip(selected) {
                reader.tick()?;
                if value != &request.values()[position] {
                    return Err(EditError::Preservation);
                }
            }
        } else if record::decode(&bytes, schema, reader)? != request.values() {
            return Err(EditError::Preservation);
        }
        reader.tick()?;
        rows.push(Row {
            bytes,
            reference: 0,
            range: 0..0,
        });
    }

    Ok(rows)
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
