//! XSD lexical domains; no SQL locale, expression evaluation or numeric rounding.
use super::{
    TemplateError, XSI,
    schema::{Field, Kind},
    tree::Node,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use jiff::civil::DateTime;
use rust_decimal::Decimal as ExactDecimal;
use tablecraft_core::values::{Decimal, Double, LegacyDateTime, Single, Value};

const MAX_NUMERIC_BYTES: usize = 128;
const HEX_PAIR_BYTES: usize = 2;
const DATE_BYTES: usize = 19;
const MAX_DATE_FRACTION_DIGITS: usize = 9;
const DATE_SEPARATOR_OFFSETS: [(usize, u8); 5] =
    [(4, b'-'), (7, b'-'), (10, b'T'), (13, b':'), (16, b':')];

pub(super) fn read(field: &Field, node: &Node) -> Result<Value, TemplateError> {
    if !node.children.is_empty() {
        return Err(TemplateError::Malformed);
    }
    let mut nil = false;
    for attribute in &node.attributes {
        if attribute.name.namespace != XSI || attribute.name.local != "nil" {
            return Err(TemplateError::Unsupported);
        }
        nil = match attribute.value.as_str() {
            "true" | "1" => true,
            "false" | "0" => false,
            _ => return Err(TemplateError::Malformed),
        };
    }
    if nil {
        return if field.nillable && node.text.is_empty() {
            Ok(Value::Null)
        } else {
            Err(TemplateError::Malformed)
        };
    }
    let text = node.text.as_str();
    let collapsed = text.trim_matches(|c| matches!(c, ' ' | '\t' | '\r' | '\n'));
    let malformed = || TemplateError::Malformed;
    let value = match field.kind {
        Kind::String => Value::Text(text.into()),
        Kind::Boolean => Value::Boolean(match collapsed {
            "true" | "1" => true,
            "false" | "0" => false,
            _ => return Err(malformed()),
        }),
        Kind::SignedByte
        | Kind::UnsignedByte
        | Kind::Short
        | Kind::UnsignedShort
        | Kind::Int
        | Kind::UnsignedInt
        | Kind::Long => {
            integer_lexical(collapsed)?;
            match field.kind {
                Kind::SignedByte => {
                    Value::Int16(collapsed.parse::<i8>().map_err(|_| malformed())?.into())
                }
                Kind::UnsignedByte => {
                    Value::Byte(u8::try_from(unsigned(collapsed)?).map_err(|_| malformed())?)
                }
                Kind::Short => Value::Int16(collapsed.parse().map_err(|_| malformed())?),
                Kind::UnsignedShort => Value::Int32(
                    u16::try_from(unsigned(collapsed)?)
                        .map_err(|_| malformed())?
                        .into(),
                ),
                Kind::Int => Value::Int32(collapsed.parse().map_err(|_| malformed())?),
                Kind::UnsignedInt => Value::Int64(
                    u32::try_from(unsigned(collapsed)?)
                        .map_err(|_| malformed())?
                        .into(),
                ),
                Kind::Long => Value::Int64(collapsed.parse().map_err(|_| malformed())?),
                _ => unreachable!(),
            }
        }
        Kind::Float => Value::Single(Single::from_float(
            float(collapsed)?.parse::<f32>().map_err(|_| malformed())?,
        )),
        Kind::Double => Value::Double(Double::from_float(
            float(collapsed)?.parse::<f64>().map_err(|_| malformed())?,
        )),
        Kind::Decimal => {
            decimal_lexical(collapsed)?;
            let value =
                ExactDecimal::from_str_exact(collapsed).map_err(|_| TemplateError::Unsupported)?;
            let canonical = value.normalize();
            let digits = canonical
                .mantissa()
                .unsigned_abs()
                .checked_ilog10()
                .map_or(1, |digits| digits as usize + 1);
            if field.total_digits.is_some_and(|limit| digits > limit)
                || field
                    .fraction_digits
                    .is_some_and(|limit| canonical.scale() as usize > limit)
            {
                return Err(malformed());
            }
            Value::Decimal(
                Decimal::new(value.mantissa(), value.scale() as u8)
                    .map_err(|_| TemplateError::Unsupported)?,
            )
        }
        Kind::DateTime => {
            // This profile materializes an unzoned legacy DATE. Do not discard a
            // supplied timezone or silently narrow a template's declared type.
            if collapsed.len() < DATE_BYTES
                || DATE_SEPARATOR_OFFSETS
                    .iter()
                    .any(|(at, expected)| collapsed.as_bytes().get(*at) != Some(expected))
                || !collapsed.is_ascii()
                || !collapsed[..DATE_BYTES]
                    .bytes()
                    .enumerate()
                    .all(|(at, byte)| {
                        byte.is_ascii_digit()
                            || DATE_SEPARATOR_OFFSETS
                                .iter()
                                .any(|(offset, expected)| at == *offset && byte == *expected)
                    })
                || collapsed.get(DATE_BYTES..).is_some_and(|fraction| {
                    !fraction.is_empty()
                        && (!fraction.starts_with('.')
                            || fraction.len() < 2
                            || fraction.len() > MAX_DATE_FRACTION_DIGITS + 1
                            || !fraction.as_bytes()[1..].iter().all(u8::is_ascii_digit))
                })
            {
                return Err(TemplateError::Unsupported);
            }
            let civil = collapsed.parse::<DateTime>().map_err(|_| malformed())?;
            Value::DateTime(
                LegacyDateTime::from_calendar(civil).map_err(|_| TemplateError::Unsupported)?,
            )
        }
        Kind::Base64 => {
            let compact: String = text
                .chars()
                .filter(|c| !matches!(c, ' ' | '\t' | '\r' | '\n'))
                .collect();
            let mut decoded = Vec::new();
            decoded
                .try_reserve_exact(compact.len())
                .map_err(|_| tablecraft_ace::InspectError::Allocation)?;
            decoded.resize(compact.len(), 0);
            let length = STANDARD
                .decode_slice(compact, &mut decoded)
                .map_err(|_| malformed())?;
            decoded.truncate(length);
            Value::Binary(decoded)
        }
        Kind::Hex => {
            if !collapsed.len().is_multiple_of(HEX_PAIR_BYTES)
                || !collapsed.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err(malformed());
            }
            let bytes = collapsed
                .as_bytes()
                .as_chunks::<HEX_PAIR_BYTES>()
                .0
                .iter()
                .map(|pair| {
                    u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16)
                        .map_err(|_| malformed())
                })
                .collect::<Result<Vec<_>, _>>()?;
            Value::Binary(bytes)
        }
    };
    let length = match &value {
        Value::Text(text) => Some(text.chars().count()),
        Value::Binary(bytes) => Some(bytes.len()),
        _ => None,
    };
    if length
        .zip(field.max_length)
        .is_some_and(|(length, maximum)| length > maximum)
    {
        return Err(malformed());
    }
    Ok(value)
}
fn integer_lexical(value: &str) -> Result<(), TemplateError> {
    let unsigned = value.strip_prefix(['+', '-']).unwrap_or(value);
    if value.len() > MAX_NUMERIC_BYTES
        || unsigned.is_empty()
        || !unsigned.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(TemplateError::Malformed);
    }
    Ok(())
}
fn decimal_lexical(value: &str) -> Result<(), TemplateError> {
    let unsigned = value.strip_prefix(['+', '-']).unwrap_or(value);
    if value.len() > MAX_NUMERIC_BYTES
        || !unsigned.bytes().any(|byte| byte.is_ascii_digit())
        || !unsigned
            .bytes()
            .all(|byte| byte.is_ascii_digit() || byte == b'.')
        || unsigned.bytes().filter(|byte| *byte == b'.').count() > 1
    {
        return Err(TemplateError::Malformed);
    }
    Ok(())
}
fn float(value: &str) -> Result<&str, TemplateError> {
    match value {
        "INF" => return Ok("inf"),
        "-INF" => return Ok("-inf"),
        "NaN" => return Ok("NaN"),
        _ => {}
    }
    if value.len() > MAX_NUMERIC_BYTES
        || !value.bytes().any(|byte| byte.is_ascii_digit())
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.' | b'e' | b'E'))
    {
        return Err(TemplateError::Malformed);
    }
    Ok(value)
}

fn unsigned(value: &str) -> Result<u64, TemplateError> {
    let value = value
        .parse::<i128>()
        .map_err(|_| TemplateError::Malformed)?;
    u64::try_from(value).map_err(|_| TemplateError::Malformed)
}
