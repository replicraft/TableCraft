use super::long_text;
use super::schema::{NativeColumn, TableSchema};
use crate::format::reader::Reader;
use crate::format::{
    data_type,
    row::{COLUMN_COUNT_BYTES, MASK_BITS},
    text::UTF16_UNIT_BYTES,
};
use crate::{
    InspectError,
    format::{
        bytes::{u16_at, u32_at},
        text,
    },
};
use tablecraft_core::values::{Currency, Double, Guid, LegacyDateTime, Single, Value};

pub(super) fn decode<R: crate::NativeReadSource>(
    bytes: &[u8],
    schema: &TableSchema,
    reader: &mut Reader<'_, R>,
) -> Result<Vec<Value>, InspectError> {
    decode_selected(bytes, schema, None, reader)
}

pub(crate) fn decode_selected<R: crate::NativeReadSource>(
    bytes: &[u8],
    schema: &TableSchema,
    selected: Option<&[usize]>,
    reader: &mut Reader<'_, R>,
) -> Result<Vec<Value>, InspectError> {
    let super::record_layout::RecordLayout {
        count,
        mask_start,
        data_end,
        offsets,
    } = super::record_layout::parse(bytes, schema, reader.budget)?;
    reader.budget.charge_allocation(
        selected.map_or(schema.columns.len(), <[usize]>::len) as u64,
        std::mem::size_of::<Value>() as u64,
    )?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(selected.map_or(schema.columns.len(), <[usize]>::len))
        .map_err(|_| InspectError::Allocation)?;
    for (position, c) in schema.columns.iter().enumerate() {
        reader.tick()?;
        if selected.is_some_and(|columns| !columns.contains(&position)) {
            continue;
        }
        let field = &c.field;
        let present = field.number < count
            && bytes[mask_start + field.number / MASK_BITS] & (1 << (field.number % MASK_BITS))
                != 0;
        let value = if field.kind == data_type::BOOLEAN && field.fixed {
            Value::Boolean(present)
        } else if !present {
            Value::Null
        } else {
            let data = if field.fixed {
                bytes
                    .get(
                        COLUMN_COUNT_BYTES + field.offset
                            ..COLUMN_COUNT_BYTES + field.offset + field.length,
                    )
                    .filter(|_| COLUMN_COUNT_BYTES + field.offset + field.length <= data_end)
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
            scalar(c, data, reader)?
        };
        values.push(value);
    }
    Ok(values)
}
fn scalar<R: crate::NativeReadSource>(
    c: &NativeColumn,
    bytes: &[u8],
    reader: &mut Reader<'_, R>,
) -> Result<Value, InspectError> {
    if super::calculated::supported(c) {
        return super::calculated::decode(c, bytes, reader.budget);
    }
    if super::complex::supported_key(c) {
        if bytes.len() != data_type::INT32_BYTES {
            return Err(InspectError::Malformed);
        }
        return Ok(Value::Int32(u32_at(bytes, 0)? as i32));
    }
    let f = &c.field;
    if f.kind == data_type::OLE {
        return Ok(match super::long_value::read(reader, bytes)? {
            Some(bytes) => Value::Binary(bytes),
            None => Value::Null,
        });
    }
    if f.kind == data_type::LONG_TEXT {
        return Ok(match long_text::read(reader, bytes)? {
            Some(text) => Value::Text(text),
            None => Value::Null,
        });
    }
    let budget = &mut *reader.budget;
    if f.kind != data_type::SHORT_TEXT
        && f.kind != data_type::SHORT_BINARY
        && bytes.len() != f.length
    {
        return Err(InspectError::Malformed);
    }
    Ok(match f.kind {
        data_type::BYTE => Value::Byte(bytes[0]),
        data_type::INT16 => Value::Int16(u16_at(bytes, 0)? as i16),
        data_type::INT32 => Value::Int32(u32_at(bytes, 0)? as i32),
        data_type::INT64 => Value::Int64(i64::from_le_bytes(
            bytes.try_into().map_err(|_| InspectError::Malformed)?,
        )),
        data_type::EXTENDED_DATE_TIME => {
            Value::DateTimeExtended(super::extended_datetime::decode(bytes)?)
        }
        data_type::CURRENCY => Value::Currency(Currency::from_scaled(i64::from_le_bytes(
            bytes.try_into().map_err(|_| InspectError::Malformed)?,
        ))),
        data_type::SINGLE => Value::Single(Single::from_bits(u32_at(bytes, 0)?)),
        data_type::DOUBLE => Value::Double(Double::from_bits(u64::from_le_bytes(
            bytes.try_into().map_err(|_| InspectError::Malformed)?,
        ))),
        data_type::DATE_TIME => Value::DateTime(
            LegacyDateTime::from_bits(u64::from_le_bytes(
                bytes.try_into().map_err(|_| InspectError::Malformed)?,
            ))
            .map_err(|_| InspectError::Malformed)?,
        ),
        data_type::NUMERIC => Value::Decimal(super::decimal::decode(bytes, f.precision, f.scale)?),
        data_type::GUID => Value::Guid(Guid::from_bytes(
            uuid::Uuid::from_bytes_le(bytes.try_into().map_err(|_| InspectError::Malformed)?)
                .into_bytes(),
        )),
        data_type::SHORT_BINARY => {
            if bytes.len() > f.length {
                return Err(InspectError::Malformed);
            }
            reader.budget.charge_allocation(bytes.len() as u64, 1)?;
            let mut binary = Vec::new();
            binary
                .try_reserve_exact(bytes.len())
                .map_err(|_| InspectError::Allocation)?;
            binary.extend_from_slice(bytes);
            Value::Binary(binary)
        }
        data_type::SHORT_TEXT => {
            let text = text::decode(bytes, true, budget)?;
            if text.encode_utf16().count() > f.length / UTF16_UNIT_BYTES {
                return Err(InspectError::Malformed);
            }
            Value::Text(text)
        }
        _ => return Err(InspectError::Unsupported),
    })
}
