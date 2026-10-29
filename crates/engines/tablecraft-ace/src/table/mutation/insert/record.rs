//! Construct a complete ordinary row from declared fields and explicit values.
mod field;
pub(in crate::table::mutation::insert) mod input;
mod layout;
mod payload;

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

/// Physical serialization only. The parent owns primary identity, defaults,
/// generated behavior, property validation and operation-specific admission.
pub(super) struct Profile<'a> {
    schema: &'a TableSchema,
    variables: Vec<usize>,
    generated: Option<usize>,
}

impl<'a> Profile<'a> {
    pub(super) fn bind(
        schema: &'a TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let variables = layout::variables(schema, budget, cancelled)?;
        Ok(Self {
            schema,
            variables,
            generated: None,
        })
    }

    pub(super) fn bind_incrementing(
        schema: &'a TableSchema,
        column: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let variables = layout::incrementing_variables(schema, column, budget, cancelled)?;
        Ok(Self {
            schema,
            variables,
            generated: Some(column),
        })
    }

    pub(super) fn prepare_input(
        &self,
        values: &[Value],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<input::Input, EditError> {
        if let Some(column) = self.generated {
            input::prepare_incrementing(
                self.schema,
                &self.variables,
                values,
                column,
                budget,
                cancelled,
            )
        } else {
            input::prepare(self.schema, &self.variables, values, budget, cancelled)
        }
    }

    pub(super) fn build(
        &self,
        values: &[Value],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, EditError> {
        check(budget, cancelled)?;
        if values.len() != self.schema.columns().len() {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        budget
            .charge_allocation(self.variables.len() as u64, size_of::<Vec<u8>>() as u64)
            .map_err(InspectError::from)?;
        let mut payloads = Vec::new();
        payloads
            .try_reserve_exact(self.variables.len())
            .map_err(|_| InspectError::Allocation)?;
        for &position in &self.variables {
            check(budget, cancelled)?;
            let payload = payload::encode(
                &self.schema.columns()[position],
                &values[position],
                budget,
                cancelled,
            )?;
            payloads.push(payload);
        }
        self.assemble(values, payloads, budget, cancelled)
    }

    pub(super) fn variables(&self) -> &[usize] {
        &self.variables
    }

    pub(super) fn build_prepared(
        &self,
        values: &[Value],
        input: input::Input,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, EditError> {
        if input.payloads.len() != self.variables.len() {
            return Err(EditError::Preservation);
        }
        budget
            .charge_allocation(input.payloads.len() as u64, size_of::<Vec<u8>>() as u64)
            .map_err(InspectError::from)?;
        let mut payloads = Vec::new();
        payloads
            .try_reserve_exact(input.payloads.len())
            .map_err(|_| InspectError::Allocation)?;
        for payload in input.payloads {
            check(budget, cancelled)?;
            let input::Payload::Bytes(bytes) = payload else {
                return Err(EditError::Preservation);
            };
            payloads.push(bytes);
        }
        self.assemble(values, payloads, budget, cancelled)
    }

    fn assemble(
        &self,
        values: &[Value],
        payloads: Vec<Vec<u8>>,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, EditError> {
        check(budget, cancelled)?;
        if values.len() != self.schema.columns().len() || payloads.len() != self.variables.len() {
            return Err(EditError::Preservation);
        }
        let mut payload_end = self.schema.layout.fixed_end;
        for payload in &payloads {
            payload_end = payload_end
                .checked_add(payload.len())
                .ok_or(InspectError::StructureLimit)?;
        }
        let mask_bytes = self.schema.layout.max_columns.div_ceil(MASK_BITS);
        let variable_footer = if self.variables.is_empty() {
            0
        } else {
            VARIABLE_OFFSET_BYTES
                .checked_mul(self.variables.len() + 1)
                .and_then(|bytes| bytes.checked_add(VARIABLE_COUNT_BYTES))
                .ok_or(InspectError::StructureLimit)?
        };
        let length = payload_end
            .checked_add(variable_footer)
            .and_then(|bytes| bytes.checked_add(mask_bytes))
            .ok_or(InspectError::StructureLimit)?;
        if length
            .checked_add(SLOT_BYTES)
            .is_none_or(|needed| needed > PAGE_BYTES - slot_offset(0))
        {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let count = u16::try_from(self.schema.layout.max_columns)
            .map_err(|_| InspectError::StructureLimit)?;
        budget
            .charge_allocation(length as u64, 1)
            .map_err(InspectError::from)?;
        budget
            .charge(Resource::Steps, length as u64)
            .map_err(InspectError::from)?;
        check(budget, cancelled)?;
        let mut row = Vec::new();
        row.try_reserve_exact(length)
            .map_err(|_| InspectError::Allocation)?;
        row.resize(length, 0);
        row[..COLUMN_COUNT_BYTES].copy_from_slice(&count.to_le_bytes());
        let mask = length - mask_bytes;
        for (position, column) in self.schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            let present = if column.fixed() {
                let width = column.storage_length();
                // Boolean has no payload; its unused offset is not an address.
                let start = if width == 0 {
                    COLUMN_COUNT_BYTES
                } else {
                    COLUMN_COUNT_BYTES + column.field.offset
                };
                field::write(column, &values[position], &mut row[start..start + width])?
            } else {
                !matches!(values[position], Value::Null)
            };
            if present {
                row[mask + column.number() / MASK_BITS] |= 1 << (column.number() % MASK_BITS);
            }
        }
        if !self.variables.is_empty() {
            let count_start = mask - VARIABLE_COUNT_BYTES;
            let variables =
                u16::try_from(self.variables.len()).map_err(|_| InspectError::StructureLimit)?;
            row[count_start..mask].copy_from_slice(&variables.to_le_bytes());
            let mut cursor = self.schema.layout.fixed_end;
            for (ordinal, payload) in payloads.into_iter().enumerate() {
                check(budget, cancelled)?;
                write_offset(&mut row, count_start, ordinal, cursor)?;
                let end = cursor + payload.len();
                row[cursor..end].copy_from_slice(&payload);
                cursor = end;
            }
            write_offset(&mut row, count_start, self.variables.len(), cursor)?;
        }
        record_layout::parse(&row, self.schema, budget)?;
        check(budget, cancelled)?;
        Ok(row)
    }
}

fn write_offset(
    row: &mut [u8],
    count_start: usize,
    ordinal: usize,
    offset: usize,
) -> Result<(), EditError> {
    let encoded = u16::try_from(offset).map_err(|_| InspectError::StructureLimit)?;
    let position = count_start
        .checked_sub(VARIABLE_OFFSET_BYTES * (ordinal + 1))
        .ok_or(InspectError::Malformed)?;
    row[position..position + VARIABLE_OFFSET_BYTES].copy_from_slice(&encoded.to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests;
