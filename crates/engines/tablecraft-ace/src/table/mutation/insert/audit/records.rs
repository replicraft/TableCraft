//! Reconstruct complete new rows independently from schema and explicit values.
use crate::table::mutation::record_insertion::proof::{layout, payload};
#[cfg(test)]
mod tests;

use super::{Row, reserve};
use crate::{
    EditError, InspectError, TableSchema, WriteBlocker,
    format::{
        data_type::{INT32, LONG_TEXT, OLE},
        reader::Reader,
    },
    table::mutation::insert::RowInsert,
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

        let bytes = crate::table::mutation::record_insertion::proof::assemble(
            schema,
            &shape,
            request.values(),
            payloads,
            end,
            resolved.map(|rows| rows[row_number].record_length),
            reader,
        )?;
        rows.push(Row {
            bytes,
            reference: 0,
            range: 0..0,
        });
    }

    Ok(rows)
}
