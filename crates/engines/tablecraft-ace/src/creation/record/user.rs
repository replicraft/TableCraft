//! One borrowed row view shares exact native encoding across construction paths.
use super::{FIXED_COLUMN_COUNT_BYTES, put16, write_fixed};
use crate::{
    InspectError,
    creation::{
        CreateDatabaseError,
        input::{Cell, Columns, Rows},
        profile::Fields,
        variable::Payload,
    },
    header::check,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_schema::ScalarType;

pub(in crate::creation) struct UserRow<'a> {
    pub fields: Fields<'a>,
    pub columns: Columns<'a>,
    pub cells: Rows<'a>,
    pub index: usize,
    pub payloads: Option<&'a [Payload<'a>]>,
    pub length: usize,
}

pub(in crate::creation) fn write(
    record: &mut [u8],
    row: UserRow<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), CreateDatabaseError> {
    if record.len() != row.length {
        return Err(CreateDatabaseError::Layout);
    }
    let fields = row.fields;
    let mask_start = record.len() - fields.count().div_ceil(u8::BITS as usize);
    put16(record, 0, fields.count())?;
    for column in 0..fields.count() {
        let field = fields.get(column);
        let Some(offset) = field.offset else {
            continue;
        };
        let start = FIXED_COLUMN_COUNT_BYTES + offset;
        let destination = record
            .get_mut(start..start + field.width)
            .ok_or(CreateDatabaseError::Layout)?;
        let scalar = row.columns.scalar(column);
        let present = match row.cells.cell(row.index, column) {
            Cell::Int32(value) => {
                if scalar != ScalarType::Int32 || destination.len() != size_of::<i32>() {
                    return Err(CreateDatabaseError::Carrier);
                }
                destination.copy_from_slice(&value.to_le_bytes());
                true
            }
            Cell::Value(Value::Null) if scalar != ScalarType::Boolean => false,
            Cell::Value(Value::Boolean(value)) if scalar == ScalarType::Boolean => {
                if !destination.is_empty() {
                    return Err(CreateDatabaseError::Layout);
                }
                *value
            }
            Cell::Value(value) => {
                write_fixed(scalar, value, destination)?;
                true
            }
        };
        if present {
            record[mask_start + column / u8::BITS as usize] |= 1 << (column % u8::BITS as usize);
        }
    }
    if let Some(payloads) = row.payloads {
        let variables = fields.variable_count();
        budget
            .charge_allocation(variables as u64, size_of::<usize>() as u64)
            .map_err(InspectError::from)?;
        let mut starts = Vec::new();
        starts
            .try_reserve_exact(variables)
            .map_err(|_| InspectError::Allocation)?;
        let mut position = fields.fixed_end();
        let mut slot = 0;
        for column in 0..fields.count() {
            if fields.get(column).offset.is_some() {
                continue;
            }
            check(budget, cancelled)?;
            starts.push(position);
            let payload = *payloads.get(slot).ok_or(CreateDatabaseError::Layout)?;
            let end = position
                .checked_add(payload.byte_len())
                .ok_or(CreateDatabaseError::Layout)?;
            payload.write(
                record
                    .get_mut(position..end)
                    .ok_or(CreateDatabaseError::Layout)?,
                budget,
                cancelled,
            )?;
            if payload.present() {
                record[mask_start + column / u8::BITS as usize] |=
                    1 << (column % u8::BITS as usize);
            }
            position = end;
            slot += 1;
        }

        // All user variable declarations are ordinary (no LVAL), hence slots
        // retain declaration order. Present empty and NULL share offsets but
        // remain distinct in the independently accumulated presence mask.
        let eod = position;
        put16(record, position, eod)?;
        position += size_of::<u16>();
        for start in starts.iter().rev() {
            check(budget, cancelled)?;
            put16(record, position, *start)?;
            position += size_of::<u16>();
        }
        put16(record, position, slot)?;
        position += size_of::<u16>();
        if position != mask_start {
            return Err(CreateDatabaseError::Layout);
        }
    }
    Ok(())
}
