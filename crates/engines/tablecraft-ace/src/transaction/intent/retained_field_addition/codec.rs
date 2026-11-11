//! Original column facts and exact defaults; native descriptor bytes are absent.
use super::{
    NativeRetainedTypedFieldAdditionIntent as Intent,
    RetainedTypedFieldAdditionIntentError as Error,
};
use crate::{
    CreateColumn, InspectError, RetainedFieldAdditionDefinition,
    format::data_type::{
        BOOLEAN, BYTE, CURRENCY, DATE_TIME, DOUBLE, GUID, INT16, INT32, INT64, NUMERIC, SHORT_TEXT,
        SINGLE,
    },
    header::check,
    identifier::{self, MAX_NAME_UNITS},
    transaction::intent::binding,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, Decimal, Double, Single, Value},
};
use tablecraft_schema::{DecimalConstraints, ScalarType};

const MAGIC: &[u8; 8] = b"TCAC0012";
const PROFILE: u32 = 12;
const TOTAL_OFFSET: usize = MAGIC.len();
const PROFILE_OFFSET: usize = TOTAL_OFFSET + size_of::<u32>();
const BEFORE_LENGTH_OFFSET: usize = PROFILE_OFFSET + size_of::<u32>();
const BINDING_OFFSET: usize = BEFORE_LENGTH_OFFSET + size_of::<u64>();
const TABLE_LENGTH_OFFSET: usize = BINDING_OFFSET + binding::DIGEST_BYTES;
const COLUMN_LENGTH_OFFSET: usize = TABLE_LENGTH_OFFSET + size_of::<u16>();
const KIND_OFFSET: usize = COLUMN_LENGTH_OFFSET + size_of::<u16>();
const KIND_RESERVED_OFFSET: usize = KIND_OFFSET + size_of::<u8>();
const TEXT_UNITS_OFFSET: usize = KIND_RESERVED_OFFSET + size_of::<u8>();
const PRECISION_OFFSET: usize = TEXT_UNITS_OFFSET + size_of::<u16>();
const SCALE_OFFSET: usize = PRECISION_OFFSET + size_of::<u8>();
const COMPRESSION_OFFSET: usize = SCALE_OFFSET + size_of::<u8>();
const COMPRESSION_RESERVED_OFFSET: usize = COMPRESSION_OFFSET + size_of::<u8>();
const DEFAULT_LENGTH_OFFSET: usize = COMPRESSION_RESERVED_OFFSET + size_of::<u8>();
const TRAILING_RESERVED_OFFSET: usize = DEFAULT_LENGTH_OFFSET + size_of::<u16>();
const HEADER_BYTES: usize = TRAILING_RESERVED_OFFSET + size_of::<u16>();
const NAME_BYTES: usize = MAX_NAME_UNITS * char::MAX.len_utf8();
const DECIMAL_COEFFICIENT_BYTES: usize = size_of::<i128>();
const DEFAULT_BYTES: usize = DECIMAL_COEFFICIENT_BYTES + size_of::<u8>();
pub(super) const MAX_BYTES: usize = HEADER_BYTES + 2 * NAME_BYTES + DEFAULT_BYTES;

fn array<const N: usize>(bytes: &[u8], at: usize) -> Result<[u8; N], Error> {
    bytes
        .get(at..at + N)
        .and_then(|value| value.try_into().ok())
        .ok_or(Error::InvalidRecord)
}

pub(super) fn validate_name(
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    if name.len() > NAME_BYTES || !identifier::valid_name(name, budget, cancelled)? {
        return Err(Error::InvalidRecord);
    }
    Ok(())
}

pub(super) fn copy_name(name: &str, budget: &mut ResourceBudget) -> Result<String, Error> {
    budget
        .charge_allocation(name.len() as u64, 1)
        .map_err(InspectError::from)?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(name.len())
        .map_err(|_| InspectError::Allocation)?;
    owned.push_str(name);
    Ok(owned)
}

fn kind(scalar: ScalarType) -> Result<u8, Error> {
    Ok(match scalar {
        ScalarType::Boolean => BOOLEAN,
        ScalarType::Byte => BYTE,
        ScalarType::Int16 => INT16,
        ScalarType::Int32 => INT32,
        ScalarType::Currency => CURRENCY,
        ScalarType::Single => SINGLE,
        ScalarType::Double => DOUBLE,
        ScalarType::DateTime => DATE_TIME,
        ScalarType::Text => SHORT_TEXT,
        ScalarType::Guid => GUID,
        ScalarType::Decimal => NUMERIC,
        ScalarType::Int64 => INT64,
        _ => return Err(Error::InvalidRecord),
    })
}

fn default_bytes(value: Option<&Value>) -> Result<([u8; DEFAULT_BYTES], usize), Error> {
    let mut bytes = [0; DEFAULT_BYTES];
    let length = match value {
        None => 0,
        Some(Value::Boolean(value)) => {
            bytes[0] = u8::from(*value);
            size_of::<u8>()
        }
        Some(Value::Byte(value)) => {
            bytes[0] = *value;
            size_of::<u8>()
        }
        Some(Value::Int16(value)) => {
            bytes[..size_of::<i16>()].copy_from_slice(&value.to_le_bytes());
            size_of::<i16>()
        }
        Some(Value::Int32(value)) => {
            bytes[..size_of::<i32>()].copy_from_slice(&value.to_le_bytes());
            size_of::<i32>()
        }
        Some(Value::Int64(value)) => {
            bytes[..size_of::<i64>()].copy_from_slice(&value.to_le_bytes());
            size_of::<i64>()
        }
        Some(Value::Currency(value)) => {
            bytes[..size_of::<i64>()].copy_from_slice(&value.scaled().to_le_bytes());
            size_of::<i64>()
        }
        Some(Value::Single(value)) => {
            bytes[..size_of::<u32>()].copy_from_slice(&value.to_bits().to_le_bytes());
            size_of::<u32>()
        }
        Some(Value::Double(value)) => {
            bytes[..size_of::<u64>()].copy_from_slice(&value.to_bits().to_le_bytes());
            size_of::<u64>()
        }
        Some(Value::Decimal(value)) => {
            bytes[..DECIMAL_COEFFICIENT_BYTES].copy_from_slice(&value.coefficient().to_le_bytes());
            bytes[DECIMAL_COEFFICIENT_BYTES] = value.scale();
            DEFAULT_BYTES
        }
        _ => return Err(Error::InvalidRecord),
    };
    Ok((bytes, length))
}

pub(super) fn encode(
    intent: &Intent,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    check(budget, cancelled)?;
    validate_name(intent.table(), budget, cancelled)?;
    validate_name(intent.column(), budget, cancelled)?;
    let column = intent.definition.column();
    let (default, default_length) = default_bytes(column.default_literal())?;
    let total = HEADER_BYTES + intent.table.len() + intent.column().len() + default_length;
    budget
        .charge_allocation(total as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, total as u64)
        .map_err(InspectError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(total)
        .map_err(|_| InspectError::Allocation)?;
    bytes.resize(HEADER_BYTES, 0);
    bytes[..MAGIC.len()].copy_from_slice(MAGIC);
    bytes[TOTAL_OFFSET..PROFILE_OFFSET].copy_from_slice(&(total as u32).to_le_bytes());
    bytes[PROFILE_OFFSET..BEFORE_LENGTH_OFFSET].copy_from_slice(&PROFILE.to_le_bytes());
    bytes[BEFORE_LENGTH_OFFSET..BINDING_OFFSET]
        .copy_from_slice(&intent.before_length.to_le_bytes());
    bytes[BINDING_OFFSET..TABLE_LENGTH_OFFSET].copy_from_slice(&intent.before_binding);
    bytes[TABLE_LENGTH_OFFSET..COLUMN_LENGTH_OFFSET]
        .copy_from_slice(&(intent.table.len() as u16).to_le_bytes());
    bytes[COLUMN_LENGTH_OFFSET..KIND_OFFSET]
        .copy_from_slice(&(intent.column().len() as u16).to_le_bytes());
    bytes[KIND_OFFSET] = kind(column.scalar_type())?;
    bytes[TEXT_UNITS_OFFSET..PRECISION_OFFSET]
        .copy_from_slice(&(column.max_text_utf16_units().unwrap_or(0) as u16).to_le_bytes());
    bytes[PRECISION_OFFSET] = column
        .decimal_constraints()
        .map_or(0, DecimalConstraints::precision);
    bytes[SCALE_OFFSET] = column
        .decimal_constraints()
        .map_or(0, DecimalConstraints::scale);
    bytes[COMPRESSION_OFFSET] = u8::from(column.unicode_compression());
    bytes[DEFAULT_LENGTH_OFFSET..TRAILING_RESERVED_OFFSET]
        .copy_from_slice(&(default_length as u16).to_le_bytes());
    bytes.extend(intent.table.as_bytes());
    bytes.extend(intent.column().as_bytes());
    bytes.extend(&default[..default_length]);
    check(budget, cancelled)?;
    Ok(bytes)
}

fn default_value(kind: u8, bytes: &[u8]) -> Result<Option<Value>, Error> {
    if bytes.is_empty() {
        return Ok(None);
    }
    Ok(Some(match kind {
        BOOLEAN if bytes.len() == size_of::<u8>() && bytes[0] <= 1 => Value::Boolean(bytes[0] != 0),
        BYTE if bytes.len() == size_of::<u8>() => Value::Byte(bytes[0]),
        INT16 if bytes.len() == size_of::<i16>() => {
            Value::Int16(i16::from_le_bytes(array(bytes, 0)?))
        }
        INT32 if bytes.len() == size_of::<i32>() => {
            Value::Int32(i32::from_le_bytes(array(bytes, 0)?))
        }
        CURRENCY if bytes.len() == size_of::<i64>() => {
            Value::Currency(Currency::from_scaled(i64::from_le_bytes(array(bytes, 0)?)))
        }
        SINGLE if bytes.len() == size_of::<u32>() => {
            Value::Single(Single::from_bits(u32::from_le_bytes(array(bytes, 0)?)))
        }
        DOUBLE if bytes.len() == size_of::<u64>() => {
            Value::Double(Double::from_bits(u64::from_le_bytes(array(bytes, 0)?)))
        }
        INT64 if bytes.len() == size_of::<i64>() => {
            Value::Int64(i64::from_le_bytes(array(bytes, 0)?))
        }
        NUMERIC if bytes.len() == DEFAULT_BYTES => Value::Decimal(
            Decimal::new(
                i128::from_le_bytes(array(bytes, 0)?),
                bytes[DECIMAL_COEFFICIENT_BYTES],
            )
            .map_err(|_| Error::InvalidRecord)?,
        ),
        _ => return Err(Error::InvalidRecord),
    }))
}

pub(super) fn decode(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Intent, Error> {
    check(budget, cancelled)?;
    if !(HEADER_BYTES..=MAX_BYTES).contains(&bytes.len())
        || bytes.get(..MAGIC.len()) != Some(MAGIC.as_slice())
        || u32::from_le_bytes(array(bytes, TOTAL_OFFSET)?) as usize != bytes.len()
        || bytes[PROFILE_OFFSET..BEFORE_LENGTH_OFFSET] != PROFILE.to_le_bytes()
        || bytes[KIND_RESERVED_OFFSET] != 0
        || bytes[COMPRESSION_RESERVED_OFFSET] != 0
        || bytes[TRAILING_RESERVED_OFFSET..HEADER_BYTES] != [0; size_of::<u16>()]
        || bytes[COMPRESSION_OFFSET] > 1
    {
        return Err(Error::InvalidRecord);
    }
    let before_length = u64::from_le_bytes(array(bytes, BEFORE_LENGTH_OFFSET)?);
    binding::retained_length(before_length)?;
    let table_length = usize::from(u16::from_le_bytes(array(bytes, TABLE_LENGTH_OFFSET)?));
    let name_length = usize::from(u16::from_le_bytes(array(bytes, COLUMN_LENGTH_OFFSET)?));
    let default_length = usize::from(u16::from_le_bytes(array(bytes, DEFAULT_LENGTH_OFFSET)?));
    if table_length > NAME_BYTES
        || name_length > NAME_BYTES
        || default_length > DEFAULT_BYTES
        || HEADER_BYTES + table_length + name_length + default_length != bytes.len()
    {
        return Err(Error::InvalidRecord);
    }
    budget
        .charge(Resource::Steps, bytes.len() as u64)
        .map_err(InspectError::from)?;
    let name_at = HEADER_BYTES + table_length;
    let value_at = name_at + name_length;
    let table =
        std::str::from_utf8(&bytes[HEADER_BYTES..name_at]).map_err(|_| Error::InvalidRecord)?;
    let name = std::str::from_utf8(&bytes[name_at..value_at]).map_err(|_| Error::InvalidRecord)?;
    validate_name(table, budget, cancelled)?;
    validate_name(name, budget, cancelled)?;
    let scalar = match bytes[KIND_OFFSET] {
        BOOLEAN => ScalarType::Boolean,
        BYTE => ScalarType::Byte,
        INT16 => ScalarType::Int16,
        INT32 => ScalarType::Int32,
        CURRENCY => ScalarType::Currency,
        SINGLE => ScalarType::Single,
        DOUBLE => ScalarType::Double,
        DATE_TIME => ScalarType::DateTime,
        SHORT_TEXT => ScalarType::Text,
        GUID => ScalarType::Guid,
        NUMERIC => ScalarType::Decimal,
        INT64 => ScalarType::Int64,
        _ => return Err(Error::InvalidRecord),
    };
    let width = usize::from(u16::from_le_bytes(array(bytes, TEXT_UNITS_OFFSET)?));
    if scalar != ScalarType::Text && (width != 0 || bytes[COMPRESSION_OFFSET] != 0)
        || scalar != ScalarType::Decimal
            && (bytes[PRECISION_OFFSET] != 0 || bytes[SCALE_OFFSET] != 0)
    {
        return Err(Error::InvalidRecord);
    }
    let default = default_value(bytes[KIND_OFFSET], &bytes[value_at..])?;
    let mut column = match scalar {
        ScalarType::Text => CreateColumn::short_text(name, width)?
            .with_allow_zero_length(true)?
            .with_unicode_compression(bytes[COMPRESSION_OFFSET] != 0)?,
        ScalarType::Decimal => CreateColumn::decimal(
            name,
            DecimalConstraints::new(bytes[PRECISION_OFFSET], bytes[SCALE_OFFSET])
                .map_err(|_| Error::InvalidRecord)?,
        ),
        _ => CreateColumn::new(name, scalar),
    }
    .with_required(false);
    if let Some(value) = &default {
        column = column.with_default_literal(value)?;
    }
    let definition = RetainedFieldAdditionDefinition::capture(column, budget, cancelled)?;
    let table = copy_name(table, budget)?;
    check(budget, cancelled)?;
    Ok(Intent {
        table,
        definition,
        before_length,
        before_binding: array(bytes, BINDING_OFFSET)?,
    })
}
