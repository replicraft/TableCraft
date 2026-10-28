//! Full-request payload choices precede every external LVAL reservation.
#[cfg(test)]
mod tests;

use super::super::records::payload::long_value::{self, Payload};
use super::super::{
    records::{layout, payload},
    reserve,
};
use crate::{
    EditError, InspectError, TableSchema, WriteBlocker,
    format::{
        data_type::{LONG_TEXT, OLE},
        page::PAGE_BYTES,
        reader::Reader,
        row::{SLOT_BYTES, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES, slot_offset},
    },
    limits::MAX_LONG_VALUE_CHUNKS,
    table::mutation::insert::RowInsert,
};
use std::io::Cursor;

const INLINE_BYTES: usize = 64;
const SINGLE_BYTES: usize = 4076;
const REFERENCE_BYTES: usize = size_of::<u32>();
const CHAIN_BYTES: usize = SINGLE_BYTES - REFERENCE_BYTES;
const DESCRIPTOR_BYTES: usize = 3 * size_of::<u32>();

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum Storage {
    Inline,
    Single,
    Chained,
}

pub(super) struct Field {
    pub column: usize,
    pub payload: Payload,
    pub storage: Storage,
}

pub(super) struct Row {
    pub fields: Vec<Field>,
    pub record_length: usize,
}

pub(super) fn derive(
    schema: &TableSchema,
    requests: &[RowInsert],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<Row>, EditError> {
    let shape = layout::capture(schema, reader.budget, reader.cancelled)?;
    let mut rows = Vec::new();
    reserve(&mut rows, requests.len(), reader)?;
    for request in requests {
        reader.tick()?;
        if request.values().len() != schema.columns().len() {
            return Err(EditError::Preservation);
        }
        let footer = if shape.variables.is_empty() {
            0
        } else {
            (shape.variables.len() + 1)
                .checked_mul(VARIABLE_OFFSET_BYTES)
                .and_then(|bytes| bytes.checked_add(VARIABLE_COUNT_BYTES))
                .ok_or(InspectError::StructureLimit)?
        };
        let mut length = shape
            .fixed_end
            .checked_add(footer)
            .and_then(|bytes| bytes.checked_add(shape.mask_bytes))
            .ok_or(InspectError::StructureLimit)?;
        let mut fields = Vec::new();
        reserve(&mut fields, shape.variables.len(), reader)?;
        for &column in &shape.variables {
            reader.tick()?;
            let field = &schema.columns()[column];
            let value = &request.values()[column];
            if !matches!(field.native_type(), LONG_TEXT | OLE) {
                let encoded = payload::encode(field, value, reader.budget, reader.cancelled)?;
                length = length
                    .checked_add(encoded.len())
                    .ok_or(InspectError::StructureLimit)?;
                continue;
            }
            let encoded = long_value::requested(field, value, reader.budget, reader.cancelled)?;
            let (storage, bytes) = match &encoded {
                Payload::Null => (Storage::Inline, 0),
                Payload::Present(bytes) if bytes.len() <= INLINE_BYTES => {
                    (Storage::Inline, DESCRIPTOR_BYTES + bytes.len())
                }
                Payload::Present(bytes) if bytes.len() <= SINGLE_BYTES => {
                    (Storage::Single, DESCRIPTOR_BYTES)
                }
                Payload::Present(bytes) => {
                    if bytes.len().div_ceil(CHAIN_BYTES) > MAX_LONG_VALUE_CHUNKS {
                        return Err(InspectError::StructureLimit.into());
                    }
                    (Storage::Chained, DESCRIPTOR_BYTES)
                }
            };
            length = length
                .checked_add(bytes)
                .ok_or(InspectError::StructureLimit)?;
            fields.push(Field {
                column,
                payload: encoded,
                storage,
            });
        }

        let maximum = PAGE_BYTES
            .checked_sub(slot_offset(0) + SLOT_BYTES)
            .ok_or(EditError::Preservation)?;
        for field in &mut fields {
            reader.tick()?;
            if length <= maximum {
                break;
            }
            if field.storage != Storage::Inline {
                continue;
            }
            if let Payload::Present(bytes) = &field.payload
                && !bytes.is_empty()
            {
                length = length
                    .checked_sub(bytes.len())
                    .ok_or(EditError::Preservation)?;
                field.storage = Storage::Single;
            }
        }
        if length > maximum {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        rows.push(Row {
            fields,
            record_length: length,
        });
    }
    Ok(rows)
}
