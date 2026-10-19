//! Exact engineering-harness carriers, not implicit SQL literal coercion.
use clap::ValueEnum;
use tablecraft_core::values::{Currency, Decimal, Double, Guid, LegacyDateTime, Single, Value};

const HEX_RADIX: u32 = 16;

#[derive(Clone, Copy, ValueEnum)]
pub enum Kind {
    Boolean,
    Byte,
    Int16,
    Int32,
    Int64,
    CurrencyScaled,
    Decimal,
    SingleBits,
    DoubleBits,
    DateBits,
    Guid,
}

pub fn parse(kind: Kind, text: &str) -> Result<Value, &'static str> {
    if text == "null" {
        return Ok(Value::Null);
    }
    let invalid = "invalid exact scalar input";
    Ok(match kind {
        Kind::Boolean => Value::Boolean(text.parse().map_err(|_| invalid)?),
        Kind::Byte => Value::Byte(text.parse().map_err(|_| invalid)?),
        Kind::Int16 => Value::Int16(text.parse().map_err(|_| invalid)?),
        Kind::Int32 => Value::Int32(text.parse().map_err(|_| invalid)?),
        Kind::Int64 => Value::Int64(text.parse().map_err(|_| invalid)?),
        Kind::CurrencyScaled => {
            Value::Currency(Currency::from_scaled(text.parse().map_err(|_| invalid)?))
        }
        Kind::Decimal => {
            let (coefficient, scale) = text.split_once(':').ok_or(invalid)?;
            Value::Decimal(
                Decimal::new(
                    coefficient.parse().map_err(|_| invalid)?,
                    scale.parse().map_err(|_| invalid)?,
                )
                .map_err(|_| invalid)?,
            )
        }
        Kind::SingleBits => Value::Single(Single::from_bits(
            u32::from_str_radix(text, HEX_RADIX).map_err(|_| invalid)?,
        )),
        Kind::DoubleBits => Value::Double(Double::from_bits(
            u64::from_str_radix(text, HEX_RADIX).map_err(|_| invalid)?,
        )),
        Kind::DateBits => Value::DateTime(
            LegacyDateTime::from_bits(u64::from_str_radix(text, HEX_RADIX).map_err(|_| invalid)?)
                .map_err(|_| invalid)?,
        ),
        Kind::Guid => Value::Guid(Guid::from_bytes(
            uuid::Uuid::parse_str(text)
                .map_err(|_| invalid)?
                .into_bytes(),
        )),
    })
}
