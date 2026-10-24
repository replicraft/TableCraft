use super::super::schema::NativeColumn;
use super::decimal;
use super::wrapper::{self, WRAPPER_BYTES};
use crate::{
    InspectError, ReadProfile,
    format::{
        data_type,
        layout::{CALCULATED_EXT_MASK, UNICODE_COMPRESSION_FLAG, UPDATABLE_FLAG},
        text::{self, UTF16_UNIT_BYTES},
    },
};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Currency, Double, LegacyDateTime, Value},
};
use tablecraft_schema::ScalarType;

const FIXED_RESULT_CAPACITY_BYTES: usize = 39;
const BOOLEAN_PAYLOAD_BYTES: usize = size_of::<u8>();
const BOOLEAN_FALSE: u8 = 0;
const BOOLEAN_TRUE: u8 = 0xff;
const COMPRESSED_CALCULATED_FLAGS: u8 = CALCULATED_EXT_MASK | UNICODE_COMPRESSION_FLAG;

pub(in crate::table) fn kind(column: &NativeColumn) -> Option<ScalarType> {
    if !matches!(
        column.read_profile(),
        ReadProfile::Ace2016 | ReadProfile::Ace2019
    ) || column.fixed()
        || column.flags() != UPDATABLE_FLAG
    {
        return None;
    }
    if column.native_type() == data_type::LONG_TEXT {
        return (column.storage_length() == 0 && column.extended_flags() == CALCULATED_EXT_MASK)
            .then_some(ScalarType::Text);
    }
    if column.native_type() == data_type::SHORT_TEXT {
        let capacity = column.storage_length().checked_sub(WRAPPER_BYTES)?;
        return (capacity <= data_type::MAX_SHORT_TEXT_BYTES
            && capacity.is_multiple_of(UTF16_UNIT_BYTES)
            && matches!(
                column.extended_flags(),
                CALCULATED_EXT_MASK | COMPRESSED_CALCULATED_FLAGS
            ))
        .then_some(ScalarType::Text);
    }
    if column.storage_length() != FIXED_RESULT_CAPACITY_BYTES
        || column.extended_flags() != CALCULATED_EXT_MASK
    {
        return None;
    }
    Some(match column.native_type() {
        data_type::BOOLEAN => ScalarType::Boolean,
        data_type::INT32 => ScalarType::Int32,
        data_type::DOUBLE => ScalarType::Double,
        data_type::CURRENCY => ScalarType::Currency,
        data_type::DATE_TIME => ScalarType::DateTime,
        data_type::NUMERIC if column.decimal_parameters() == Some((0, 0)) => ScalarType::Decimal,
        _ => return None,
    })
}

pub(in crate::table) fn supported(column: &NativeColumn) -> bool {
    kind(column).is_some()
}

pub(in crate::table) fn decode(
    column: &NativeColumn,
    bytes: &[u8],
    budget: &mut ResourceBudget,
) -> Result<Value, InspectError> {
    let kind = kind(column).ok_or(InspectError::Unsupported)?;
    let bytes = wrapper::payload(bytes)?;
    if kind == ScalarType::Text {
        let capacity = column
            .storage_length()
            .checked_sub(WRAPPER_BYTES)
            .ok_or(InspectError::Unsupported)?
            / UTF16_UNIT_BYTES;
        let value = text::decode(bytes, true, budget)?;
        if value.encode_utf16().count() > capacity {
            return Err(InspectError::Malformed);
        }
        return Ok(Value::Text(value));
    }
    if kind == ScalarType::Boolean {
        // The independent stored Boolean reader treats a qualified empty
        // wrapper as false. An absent row-mask value remains NULL upstream.
        return Ok(Value::Boolean(match bytes {
            [] => false,
            [BOOLEAN_FALSE] => false,
            [BOOLEAN_TRUE] => true,
            _ if bytes.len() != BOOLEAN_PAYLOAD_BYTES => return Err(InspectError::Malformed),
            _ => return Err(InspectError::Unsupported),
        }));
    }
    if bytes.is_empty() {
        return Ok(Value::Null);
    }
    Ok(match kind {
        ScalarType::Decimal => Value::Decimal(decimal::decode(bytes)?),
        ScalarType::Int32 => Value::Int32(i32::from_le_bytes(
            bytes.try_into().map_err(|_| InspectError::Malformed)?,
        )),
        ScalarType::Double => Value::Double(Double::from_bits(u64::from_le_bytes(
            bytes.try_into().map_err(|_| InspectError::Malformed)?,
        ))),
        ScalarType::Currency => Value::Currency(Currency::from_scaled(i64::from_le_bytes(
            bytes.try_into().map_err(|_| InspectError::Malformed)?,
        ))),
        ScalarType::DateTime => Value::DateTime(
            LegacyDateTime::from_bits(u64::from_le_bytes(
                bytes.try_into().map_err(|_| InspectError::Malformed)?,
            ))
            .map_err(|_| InspectError::Malformed)?,
        ),
        _ => return Err(InspectError::Unsupported),
    })
}
