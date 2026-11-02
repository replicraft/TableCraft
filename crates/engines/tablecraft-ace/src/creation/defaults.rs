//! Bounded canonical constant declarations, without an expression evaluator.
mod date;

use super::CreateDatabaseError;
use crate::header::check;
use tablecraft_core::{limits::ResourceBudget, values::Value};

const MAX_NUMBER_BYTES: usize = 21;
const MAX_EXPRESSION_UNITS: usize = 255;
const DECIMAL_RADIX: u64 = 10;
const CURRENCY_SCALE: u64 = 10_000;
const CURRENCY_FRACTION_DIGITS: usize = 4;
const QUOTE: u16 = b'"' as u16;

pub(super) struct Literal {
    units: [u16; MAX_EXPRESSION_UNITS],
    length: usize,
}

impl Literal {
    pub fn capture(
        value: &Value,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CreateDatabaseError> {
        check(budget, cancelled)?;
        match value {
            Value::Null => Self::word(b"Null", budget, cancelled),
            Value::Boolean(true) => Self::word(b"True", budget, cancelled),
            Value::Boolean(false) => Self::word(b"False", budget, cancelled),
            Value::Int32(value) => Self::number(i64::from(*value), false, budget, cancelled),
            Value::Int64(value) => Self::number(*value, false, budget, cancelled),
            Value::Currency(value) => Self::number(value.scaled(), true, budget, cancelled),
            Value::DateTime(value) => {
                let literal = date::capture(*value, budget, cancelled)?;
                Self::word(literal.bytes(), budget, cancelled)
            }
            Value::Text(value) => Self::text(value, budget, cancelled),
            _ => Err(CreateDatabaseError::Carrier),
        }
    }

    fn word(
        word: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CreateDatabaseError> {
        let mut units = [0; MAX_EXPRESSION_UNITS];
        let mut length = 0;
        for byte in word {
            Self::push(&mut units, &mut length, u16::from(*byte), budget, cancelled)?;
        }
        Ok(Self { units, length })
    }

    fn number(
        value: i64,
        currency: bool,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CreateDatabaseError> {
        let mut bytes = [0; MAX_NUMBER_BYTES];
        let mut start = MAX_NUMBER_BYTES;
        let mut magnitude = value.unsigned_abs();
        if currency {
            // The stored scaled integer is authoritative, including MIN. No
            // floating point conversion, locale or display formatting is used.
            let mut fraction = magnitude % CURRENCY_SCALE;
            magnitude /= CURRENCY_SCALE;
            for _ in 0..CURRENCY_FRACTION_DIGITS {
                check(budget, cancelled)?;
                start -= 1;
                bytes[start] = b'0' + (fraction % DECIMAL_RADIX) as u8;
                fraction /= DECIMAL_RADIX;
            }
            check(budget, cancelled)?;
            start -= 1;
            bytes[start] = b'.';
        }
        loop {
            check(budget, cancelled)?;
            start -= 1;
            bytes[start] = b'0' + (magnitude % DECIMAL_RADIX) as u8;
            magnitude /= DECIMAL_RADIX;
            if magnitude == 0 {
                break;
            }
        }
        if value < 0 {
            check(budget, cancelled)?;
            start -= 1;
            bytes[start] = b'-';
        }
        check(budget, cancelled)?;
        Self::word(&bytes[start..], budget, cancelled)
    }

    fn text(
        text: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CreateDatabaseError> {
        // The same expression ceiling is enforced by the actual omitted INSERT
        // resolver. Bounded stack storage avoids a hidden formatting allocation.
        let mut units = [0; MAX_EXPRESSION_UNITS];
        let mut length = 0;
        Self::push(&mut units, &mut length, QUOTE, budget, cancelled)?;
        for unit in text.encode_utf16() {
            Self::push(&mut units, &mut length, unit, budget, cancelled)?;
            if unit == QUOTE {
                Self::push(&mut units, &mut length, QUOTE, budget, cancelled)?;
            }
        }
        Self::push(&mut units, &mut length, QUOTE, budget, cancelled)?;
        Ok(Self { units, length })
    }

    fn push(
        units: &mut [u16; MAX_EXPRESSION_UNITS],
        length: &mut usize,
        unit: u16,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), CreateDatabaseError> {
        check(budget, cancelled)?;
        let destination = units
            .get_mut(*length)
            .ok_or(CreateDatabaseError::Declaration)?;
        *destination = unit;
        *length += 1;
        Ok(())
    }

    pub fn units(&self) -> impl Iterator<Item = u16> + '_ {
        self.units[..self.length].iter().copied()
    }

    pub fn payload_bytes(&self) -> usize {
        self.length * size_of::<u16>()
    }
}
