//! Exact typed protocol values; SQL coercions and generated markers are absent.
use super::error::{Error, input};
use tablecraft_ace::InspectError;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Currency, Decimal, Double, ExtendedDateTime, Guid, LegacyDateTime, Single, Value},
};

const HEX_RADIX: u32 = 16;
const HEX_DIGITS_PER_BYTE: usize = 2;
const EXTENDED_TEXT_BYTES: usize = "0001-01-01T00:00:00.0000000".len();
const NANOS_PER_TICK: u32 = 100;
const EXTENDED_SEPARATORS: [(usize, u8); 6] = [
    (4, b'-'),
    (7, b'-'),
    (10, b'T'),
    (13, b':'),
    (16, b':'),
    (19, b'.'),
];

pub(super) fn value(
    kind: &str,
    data: &str,
    null_word: bool,
    budget: &mut ResourceBudget,
) -> Result<Value, Error> {
    budget
        .charge(Resource::Steps, (kind.len() + data.len()) as u64)
        .map_err(InspectError::from)?;
    if null_word && data == "null" {
        // Validate the declared kind even for NULL, so misspelled kinds do not
        // turn into accepted values on empty tables or nullable assignments.
        if !fixed_kind(kind) && !matches!(kind, "text" | "binary") {
            return Err(input("Unknown fixed update KIND"));
        }
        return Ok(Value::Null);
    }
    let invalid = || input("Invalid exact typed value");
    Ok(match kind {
        "null" if data.is_empty() && !null_word => Value::Null,
        "boolean" => Value::Boolean(data.parse().map_err(|_| invalid())?),
        "byte" => Value::Byte(data.parse().map_err(|_| invalid())?),
        "int16" => Value::Int16(data.parse().map_err(|_| invalid())?),
        "int32" => Value::Int32(data.parse().map_err(|_| invalid())?),
        "int64" => Value::Int64(data.parse().map_err(|_| invalid())?),
        "currency-scaled" => {
            Value::Currency(Currency::from_scaled(data.parse().map_err(|_| invalid())?))
        }
        "decimal" => {
            let (coefficient, scale) = data.split_once(':').ok_or_else(invalid)?;
            Value::Decimal(
                Decimal::new(
                    coefficient.parse().map_err(|_| invalid())?,
                    scale.parse().map_err(|_| invalid())?,
                )
                .map_err(|_| invalid())?,
            )
        }
        "single-bits" => Value::Single(Single::from_bits(
            u32::from_str_radix(data, HEX_RADIX).map_err(|_| invalid())?,
        )),
        "double-bits" => Value::Double(Double::from_bits(
            u64::from_str_radix(data, HEX_RADIX).map_err(|_| invalid())?,
        )),
        "date-bits" => Value::DateTime(
            LegacyDateTime::from_bits(u64::from_str_radix(data, HEX_RADIX).map_err(|_| invalid())?)
                .map_err(|_| invalid())?,
        ),
        "guid" => Value::Guid(Guid::from_bytes(
            uuid::Uuid::parse_str(data)
                .map_err(|_| invalid())?
                .into_bytes(),
        )),
        "extended-date" => Value::DateTimeExtended(extended(data)?),
        "text" => {
            budget
                .charge_allocation(data.len() as u64, 1)
                .map_err(InspectError::from)?;
            let mut text = String::new();
            text.try_reserve_exact(data.len())
                .map_err(|_| InspectError::Allocation)?;
            text.push_str(data);
            Value::Text(text)
        }
        "binary" => Value::Binary(hex(data, budget)?),
        _ => return Err(input("Unknown KIND or invalid NULL payload")),
    })
}

pub(super) fn update(kind: &str, data: &str, budget: &mut ResourceBudget) -> Result<Value, Error> {
    if let Some(literal) = data.strip_prefix("value:") {
        return value(kind, literal, false, budget);
    }
    value(kind, data, true, budget)
}

fn fixed_kind(kind: &str) -> bool {
    matches!(
        kind,
        "boolean"
            | "byte"
            | "int16"
            | "int32"
            | "int64"
            | "currency-scaled"
            | "decimal"
            | "single-bits"
            | "double-bits"
            | "date-bits"
            | "guid"
            | "extended-date"
    )
}

pub(super) fn hex(data: &str, budget: &mut ResourceBudget) -> Result<Vec<u8>, Error> {
    if !data.len().is_multiple_of(HEX_DIGITS_PER_BYTE) {
        return Err(input("Expected complete hexadecimal byte pairs"));
    }
    let count = data.len() / HEX_DIGITS_PER_BYTE;
    budget
        .charge_allocation(count as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, data.len() as u64)
        .map_err(InspectError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    for pair in data.as_bytes().as_chunks::<HEX_DIGITS_PER_BYTE>().0 {
        let high = digit(pair[0]).ok_or_else(|| input("Invalid hexadecimal byte"))?;
        let low = digit(pair[1]).ok_or_else(|| input("Invalid hexadecimal byte"))?;
        bytes.push((high << (u8::BITS / 2)) | low);
    }
    Ok(bytes)
}

fn digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn extended(data: &str) -> Result<ExtendedDateTime, Error> {
    if data.len() != EXTENDED_TEXT_BYTES
        || !data.as_bytes().iter().enumerate().all(|(position, byte)| {
            match EXTENDED_SEPARATORS
                .iter()
                .find(|(offset, _)| *offset == position)
            {
                Some((_, expected)) => byte == expected,
                None => byte.is_ascii_digit(),
            }
        })
    {
        return Err(input(
            "Expected YYYY-MM-DDTHH:MM:SS.fffffff without a timezone",
        ));
    }
    let date: jiff::civil::DateTime = data
        .parse()
        .map_err(|_| input("Invalid extended-date civil value"))?;
    ExtendedDateTime::new(
        date.year() as u16,
        date.month() as u8,
        date.day() as u8,
        date.hour() as u8,
        date.minute() as u8,
        date.second() as u8,
        date.subsec_nanosecond() as u32 / NANOS_PER_TICK,
    )
    .map_err(|_| input("Invalid extended-date civil value"))
}
