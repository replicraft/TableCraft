use super::schema::{NativeColumn, TableSchema};
use crate::{
    InspectError,
    format::{
        bytes::{u16_at, u32_at},
        text,
    },
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Double, Single, Value},
};

pub(super) fn decode(
    bytes: &[u8],
    schema: &TableSchema,
    budget: &mut ResourceBudget,
) -> Result<Vec<Value>, InspectError> {
    let count = usize::from(u16_at(bytes, 0)?);
    // Older rows/column deletion need a separate qualified evolution profile.
    if count != schema.layout.max_columns {
        return Err(InspectError::Unsupported);
    }
    let mask_len = count.div_ceil(8);
    let mask_start = bytes
        .len()
        .checked_sub(mask_len)
        .ok_or(InspectError::Truncated)?;
    let (data_end, offsets) = if schema.layout.variable_slots == 0 {
        (mask_start, Vec::new())
    } else {
        let count_pos = mask_start.checked_sub(2).ok_or(InspectError::Truncated)?;
        let variables = usize::from(u16_at(bytes, count_pos)?);
        if variables == 0 || variables > schema.layout.variable_slots {
            return Err(InspectError::Unsupported);
        }
        let tail = count_pos
            .checked_sub(2 * (variables + 1))
            .ok_or(InspectError::Truncated)?;
        budget.charge_allocation((variables + 1) as u64, std::mem::size_of::<usize>() as u64)?;
        let mut offsets = Vec::new();
        offsets
            .try_reserve_exact(variables + 1)
            .map_err(|_| InspectError::Allocation)?;
        let mut previous = schema.layout.fixed_end;
        for i in 0..=variables {
            let next = usize::from(u16_at(bytes, count_pos - 2 * (i + 1))?);
            if next < previous || next > tail {
                return Err(InspectError::Malformed);
            }
            offsets.push(next);
            previous = next;
        }
        (tail, offsets)
    };
    if schema.layout.fixed_end > data_end {
        return Err(InspectError::Malformed);
    }
    budget.charge_allocation(
        schema.columns.len() as u64,
        std::mem::size_of::<Value>() as u64,
    )?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(schema.columns.len())
        .map_err(|_| InspectError::Allocation)?;
    for c in &schema.columns {
        let field = &c.field;
        let present = bytes[mask_start + field.number / 8] & (1 << (field.number % 8)) != 0;
        let value = if field.kind == 1 {
            Value::Boolean(present)
        } else if !present {
            Value::Null
        } else {
            let data = if field.fixed {
                bytes
                    .get(2 + field.offset..2 + field.offset + field.length)
                    .filter(|_| 2 + field.offset + field.length <= data_end)
                    .ok_or(InspectError::Malformed)?
            } else {
                let start = *offsets
                    .get(field.variable)
                    .ok_or(InspectError::Unsupported)?;
                let end = *offsets
                    .get(field.variable + 1)
                    .ok_or(InspectError::Unsupported)?;
                &bytes[start..end]
            };
            scalar(c, data, budget)?
        };
        values.push(value);
    }
    Ok(values)
}
fn scalar(
    c: &NativeColumn,
    bytes: &[u8],
    budget: &mut ResourceBudget,
) -> Result<Value, InspectError> {
    let f = &c.field;
    if f.kind != 10 && f.kind != 9 && bytes.len() != f.length {
        return Err(InspectError::Malformed);
    }
    Ok(match f.kind {
        2 => Value::Byte(bytes[0]),
        3 => Value::Int16(u16_at(bytes, 0)? as i16),
        4 => Value::Int32(u32_at(bytes, 0)? as i32),
        5 => Value::Currency(Currency::from_scaled(i64::from_le_bytes(
            bytes.try_into().map_err(|_| InspectError::Malformed)?,
        ))),
        6 => Value::Single(Single::from_bits(u32_at(bytes, 0)?)),
        7 => Value::Double(Double::from_bits(u64::from_le_bytes(
            bytes.try_into().map_err(|_| InspectError::Malformed)?,
        ))),
        9 => {
            if bytes.len() > f.length {
                return Err(InspectError::Malformed);
            }
            budget.charge_allocation(bytes.len() as u64, 1)?;
            let mut binary = Vec::new();
            binary
                .try_reserve_exact(bytes.len())
                .map_err(|_| InspectError::Allocation)?;
            binary.extend_from_slice(bytes);
            Value::Binary(binary)
        }
        10 => {
            let text = text::decode(bytes, true, budget)?;
            if text.encode_utf16().count() > f.length / 2 {
                return Err(InspectError::Malformed);
            }
            Value::Text(text)
        }
        _ => return Err(InspectError::Unsupported),
    })
}
