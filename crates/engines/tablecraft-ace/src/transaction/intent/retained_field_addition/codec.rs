//! Original column facts and exact defaults; native descriptor bytes are absent.
use super::{
    NativeRetainedTypedFieldAdditionIntent as Intent,
    RetainedTypedFieldAdditionIntentError as Error,
};
use crate::{
    CreateColumn, InspectError, RetainedFieldAdditionDefinition, header::check,
    transaction::intent::binding,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, Decimal, Double, Single, Value},
};
use tablecraft_schema::{DecimalConstraints, ScalarType};

const MAGIC: &[u8; 8] = b"TCAC0012";
const HEADER: usize = 72;
const NAME_BYTES: usize = 64 * char::MAX.len_utf8();
const DEFAULT_BYTES: usize = size_of::<i128>() + size_of::<u8>();
pub(super) const MAX_BYTES: usize = HEADER + 2 * NAME_BYTES + DEFAULT_BYTES;

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
    if name.len() > NAME_BYTES || !crate::identifier::valid_name(name, budget, cancelled)? {
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
        ScalarType::Boolean => 1,
        ScalarType::Byte => 2,
        ScalarType::Int16 => 3,
        ScalarType::Int32 => 4,
        ScalarType::Currency => 5,
        ScalarType::Single => 6,
        ScalarType::Double => 7,
        ScalarType::DateTime => 8,
        ScalarType::Text => 10,
        ScalarType::Guid => 15,
        ScalarType::Decimal => 16,
        ScalarType::Int64 => 19,
        _ => return Err(Error::InvalidRecord),
    })
}

fn default_bytes(value: Option<&Value>) -> Result<([u8; DEFAULT_BYTES], usize), Error> {
    let mut bytes = [0; DEFAULT_BYTES];
    let length = match value {
        None => 0,
        Some(Value::Boolean(value)) => {
            bytes[0] = u8::from(*value);
            1
        }
        Some(Value::Byte(value)) => {
            bytes[0] = *value;
            1
        }
        Some(Value::Int16(value)) => {
            bytes[..2].copy_from_slice(&value.to_le_bytes());
            2
        }
        Some(Value::Int32(value)) => {
            bytes[..4].copy_from_slice(&value.to_le_bytes());
            4
        }
        Some(Value::Int64(value)) => {
            bytes[..8].copy_from_slice(&value.to_le_bytes());
            8
        }
        Some(Value::Currency(value)) => {
            bytes[..8].copy_from_slice(&value.scaled().to_le_bytes());
            8
        }
        Some(Value::Single(value)) => {
            bytes[..4].copy_from_slice(&value.to_bits().to_le_bytes());
            4
        }
        Some(Value::Double(value)) => {
            bytes[..8].copy_from_slice(&value.to_bits().to_le_bytes());
            8
        }
        Some(Value::Decimal(value)) => {
            bytes[..16].copy_from_slice(&value.coefficient().to_le_bytes());
            bytes[16] = value.scale();
            17
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
    let total = HEADER + intent.table.len() + intent.column().len() + default_length;
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
    bytes.extend(MAGIC);
    bytes.extend((total as u32).to_le_bytes());
    bytes.extend([12, 0, 0, 0]);
    bytes.extend(intent.before_length.to_le_bytes());
    bytes.extend(intent.before_binding);
    bytes.extend((intent.table.len() as u16).to_le_bytes());
    bytes.extend((intent.column().len() as u16).to_le_bytes());
    bytes.push(kind(column.scalar_type())?);
    bytes.push(0);
    bytes.extend((column.max_text_utf16_units().unwrap_or(0) as u16).to_le_bytes());
    bytes.push(
        column
            .decimal_constraints()
            .map_or(0, DecimalConstraints::precision),
    );
    bytes.push(
        column
            .decimal_constraints()
            .map_or(0, DecimalConstraints::scale),
    );
    bytes.push(u8::from(column.unicode_compression()));
    bytes.push(0);
    bytes.extend((default_length as u16).to_le_bytes());
    bytes.extend([0; 2]);
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
        1 if bytes.len() == 1 && bytes[0] <= 1 => Value::Boolean(bytes[0] != 0),
        2 if bytes.len() == 1 => Value::Byte(bytes[0]),
        3 if bytes.len() == 2 => Value::Int16(i16::from_le_bytes(array(bytes, 0)?)),
        4 if bytes.len() == 4 => Value::Int32(i32::from_le_bytes(array(bytes, 0)?)),
        5 if bytes.len() == 8 => {
            Value::Currency(Currency::from_scaled(i64::from_le_bytes(array(bytes, 0)?)))
        }
        6 if bytes.len() == 4 => {
            Value::Single(Single::from_bits(u32::from_le_bytes(array(bytes, 0)?)))
        }
        7 if bytes.len() == 8 => {
            Value::Double(Double::from_bits(u64::from_le_bytes(array(bytes, 0)?)))
        }
        19 if bytes.len() == 8 => Value::Int64(i64::from_le_bytes(array(bytes, 0)?)),
        16 if bytes.len() == 17 => Value::Decimal(
            Decimal::new(i128::from_le_bytes(array(bytes, 0)?), bytes[16])
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
    if !(HEADER..=MAX_BYTES).contains(&bytes.len())
        || bytes.get(..8) != Some(MAGIC.as_slice())
        || u32::from_le_bytes(array(bytes, 8)?) as usize != bytes.len()
        || bytes[12..16] != [12, 0, 0, 0]
        || bytes[61] != 0
        || bytes[67] != 0
        || bytes[70..72] != [0, 0]
        || bytes[66] > 1
    {
        return Err(Error::InvalidRecord);
    }
    let before_length = u64::from_le_bytes(array(bytes, 16)?);
    binding::retained_length(before_length)?;
    let table_length = usize::from(u16::from_le_bytes(array(bytes, 56)?));
    let name_length = usize::from(u16::from_le_bytes(array(bytes, 58)?));
    let default_length = usize::from(u16::from_le_bytes(array(bytes, 68)?));
    if table_length > NAME_BYTES
        || name_length > NAME_BYTES
        || default_length > DEFAULT_BYTES
        || HEADER + table_length + name_length + default_length != bytes.len()
    {
        return Err(Error::InvalidRecord);
    }
    budget
        .charge(Resource::Steps, bytes.len() as u64)
        .map_err(InspectError::from)?;
    let name_at = HEADER + table_length;
    let value_at = name_at + name_length;
    let table = std::str::from_utf8(&bytes[HEADER..name_at]).map_err(|_| Error::InvalidRecord)?;
    let name = std::str::from_utf8(&bytes[name_at..value_at]).map_err(|_| Error::InvalidRecord)?;
    validate_name(table, budget, cancelled)?;
    validate_name(name, budget, cancelled)?;
    let scalar = match bytes[60] {
        1 => ScalarType::Boolean,
        2 => ScalarType::Byte,
        3 => ScalarType::Int16,
        4 => ScalarType::Int32,
        5 => ScalarType::Currency,
        6 => ScalarType::Single,
        7 => ScalarType::Double,
        8 => ScalarType::DateTime,
        10 => ScalarType::Text,
        15 => ScalarType::Guid,
        16 => ScalarType::Decimal,
        19 => ScalarType::Int64,
        _ => return Err(Error::InvalidRecord),
    };
    let width = usize::from(u16::from_le_bytes(array(bytes, 62)?));
    if scalar != ScalarType::Text && (width != 0 || bytes[66] != 0)
        || scalar != ScalarType::Decimal && (bytes[64] != 0 || bytes[65] != 0)
    {
        return Err(Error::InvalidRecord);
    }
    let default = default_value(bytes[60], &bytes[value_at..])?;
    let mut column = match scalar {
        ScalarType::Text => CreateColumn::short_text(name, width)?
            .with_allow_zero_length(true)?
            .with_unicode_compression(bytes[66] != 0)?,
        ScalarType::Decimal => CreateColumn::decimal(
            name,
            DecimalConstraints::new(bytes[64], bytes[65]).map_err(|_| Error::InvalidRecord)?,
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
        before_binding: array(bytes, 24)?,
    })
}
