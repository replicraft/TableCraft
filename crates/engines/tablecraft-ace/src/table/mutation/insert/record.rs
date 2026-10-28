//! Construct a complete ordinary row from declared fields and requested values.
use crate::{
    EditError, InspectError, TableSchema, WriteBlocker,
    format::{
        data_type::{INT32, MAX_SHORT_BINARY_BYTES, SHORT_BINARY},
        layout::{FIXED_FLAG, UPDATABLE_FLAG},
        page::PAGE_BYTES,
        row::{
            COLUMN_COUNT_BYTES, MASK_BITS, SLOT_BYTES, VARIABLE_COUNT_BYTES, VARIABLE_OFFSET_BYTES,
            slot_offset,
        },
    },
    header::check,
    table::{
        mutation::{fixed::encode_fixed, slot::storage},
        record_layout,
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

const FIELDS: usize = 2;
const VARIABLE_FIELDS: usize = 1;
const ID_BYTES: usize = size_of::<i32>();

/// A physical row codec, not table/property/index or INSERT write admission.
/// The parent must reject defaults and active/generated behavior separately.
pub(super) struct Profile<'a> {
    schema: &'a TableSchema,
    id: usize,
    binary: usize,
    id_start: usize,
    fixed_end: usize,
    mask_bytes: usize,
}

impl<'a> Profile<'a> {
    pub(super) fn bind(
        schema: &'a TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        check(budget, cancelled)?;
        if schema.columns().len() != FIELDS
            || schema.layout.max_columns != FIELDS
            || schema.layout.variable_slots != VARIABLE_FIELDS
        {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        for (position, field) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            if field.number() != position {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
        }
        let (id, binary) = match (
            schema.columns()[0].native_type(),
            schema.columns()[1].native_type(),
        ) {
            (INT32, SHORT_BINARY) => (0, 1),
            (SHORT_BINARY, INT32) => (1, 0),
            _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
        };
        let fixed = &schema.columns()[id];
        let variable = &schema.columns()[binary];
        // Exact flags reject AutoNumber, Hyperlink, calculated and complex
        // extensions. Names do not determine either field's physical role.
        if !fixed.fixed()
            || fixed.scalar_type() != Some(ScalarType::Int32)
            || fixed.storage_length() != ID_BYTES
            || fixed.flags() != (FIXED_FLAG | UPDATABLE_FLAG)
            || fixed.extended_flags() != 0
            || fixed.complex_type_id().is_some()
            || variable.fixed()
            || variable.flags() != UPDATABLE_FLAG
            || variable.extended_flags() != 0
            || variable.complex_type_id().is_some()
            || variable.storage_length() == 0
            || variable.storage_length() > MAX_SHORT_BINARY_BYTES
        {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        if variable.field.variable != 0 {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let id_start = COLUMN_COUNT_BYTES
            .checked_add(fixed.field.offset)
            .ok_or(InspectError::StructureLimit)?;
        let fixed_end = id_start
            .checked_add(ID_BYTES)
            .ok_or(InspectError::StructureLimit)?;
        if schema.layout.fixed_end != fixed_end {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let mask_bytes = schema.layout.max_columns.div_ceil(MASK_BITS);
        check(budget, cancelled)?;
        Ok(Self {
            schema,
            id,
            binary,
            id_start,
            fixed_end,
            mask_bytes,
        })
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
        let id = encode_fixed::<ID_BYTES>(ScalarType::Int32, &values[self.id])?;
        let column = &self.schema.columns()[self.binary];
        // Reuse only carrier/declared-width validation. UPDATE slot admission
        // and existing-span selection grant no authority to construct a row.
        storage::validate(column, &values[self.binary])?;
        let (payload, binary_present) = match &values[self.binary] {
            Value::Null => (&[][..], false),
            Value::Binary(bytes) => (bytes.as_slice(), true),
            _ => return Err(EditError::Unsupported(WriteBlocker::Column)),
        };
        let payload_end = self
            .fixed_end
            .checked_add(payload.len())
            .ok_or(InspectError::StructureLimit)?;
        let footer = VARIABLE_OFFSET_BYTES
            .checked_mul(VARIABLE_FIELDS + 1)
            .and_then(|length| length.checked_add(VARIABLE_COUNT_BYTES))
            .and_then(|length| length.checked_add(self.mask_bytes))
            .ok_or(InspectError::StructureLimit)?;
        let length = payload_end
            .checked_add(footer)
            .ok_or(InspectError::StructureLimit)?;
        if length
            .checked_add(SLOT_BYTES)
            .is_none_or(|needed| needed > PAGE_BYTES - slot_offset(0))
        {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let count = u16::try_from(self.schema.layout.max_columns)
            .map_err(|_| InspectError::StructureLimit)?;
        let start = u16::try_from(self.fixed_end).map_err(|_| InspectError::StructureLimit)?;
        let end = u16::try_from(payload_end).map_err(|_| InspectError::StructureLimit)?;
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
        row[self.id_start..self.fixed_end].copy_from_slice(&id);
        row[self.fixed_end..payload_end].copy_from_slice(payload);
        let mask_start = length - self.mask_bytes;
        let count_start = mask_start - VARIABLE_COUNT_BYTES;
        row[count_start..mask_start].copy_from_slice(&(VARIABLE_FIELDS as u16).to_le_bytes());
        row[count_start - VARIABLE_OFFSET_BYTES..count_start].copy_from_slice(&start.to_le_bytes());
        row[count_start - 2 * VARIABLE_OFFSET_BYTES..count_start - VARIABLE_OFFSET_BYTES]
            .copy_from_slice(&end.to_le_bytes());
        let id_number = self.schema.columns()[self.id].number();
        row[mask_start + id_number / MASK_BITS] |= 1 << (id_number % MASK_BITS);
        if binary_present {
            let number = column.number();
            row[mask_start + number / MASK_BITS] |= 1 << (number % MASK_BITS);
        }
        record_layout::parse(&row, self.schema, budget)?;
        check(budget, cancelled)?;
        Ok(row)
    }
}

#[cfg(test)]
mod tests;
