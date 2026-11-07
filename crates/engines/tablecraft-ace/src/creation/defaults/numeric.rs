//! Bounded, locale-independent numeric DefaultValue spellings.
use super::super::CreateDatabaseError;
use crate::{InspectError, header::check};
use std::fmt::{self, Write};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Decimal, Value},
};

const NUMBER_BYTES: usize = 64;
const RADIX: u128 = 10;

pub(super) struct Number {
    bytes: [u8; NUMBER_BYTES],
    length: usize,
}

impl Write for Number {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.length.checked_add(text.len()).ok_or(fmt::Error)?;
        self.bytes
            .get_mut(self.length..end)
            .ok_or(fmt::Error)?
            .copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}

impl Number {
    pub(super) fn capture(
        value: &Value,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, CreateDatabaseError> {
        check(budget, cancelled)?;
        // Formatting and its exact reparse are bounded by this stack buffer.
        budget
            .charge(Resource::Steps, (2 * NUMBER_BYTES) as u64)
            .map_err(InspectError::from)?;
        let mut number = Self {
            bytes: [0; NUMBER_BYTES],
            length: 0,
        };
        match value {
            Value::Single(value) => {
                let scalar = f32::from_bits(value.to_bits());
                if !scalar.is_finite() {
                    return Err(CreateDatabaseError::Carrier);
                }
                write!(number, "{scalar:e}").map_err(|_| CreateDatabaseError::Carrier)?;
                let text = std::str::from_utf8(number.bytes())
                    .map_err(|_| CreateDatabaseError::Carrier)?;
                if text.parse::<f32>().map(f32::to_bits) != Ok(value.to_bits()) {
                    return Err(CreateDatabaseError::Carrier);
                }
            }
            Value::Double(value) => {
                let scalar = f64::from_bits(value.to_bits());
                if !scalar.is_finite() {
                    return Err(CreateDatabaseError::Carrier);
                }
                write!(number, "{scalar:e}").map_err(|_| CreateDatabaseError::Carrier)?;
                let text = std::str::from_utf8(number.bytes())
                    .map_err(|_| CreateDatabaseError::Carrier)?;
                if text.parse::<f64>().map(f64::to_bits) != Ok(value.to_bits()) {
                    return Err(CreateDatabaseError::Carrier);
                }
            }
            Value::Decimal(value) => number.decimal(*value)?,
            _ => return Err(CreateDatabaseError::Carrier),
        }
        check(budget, cancelled)?;
        Ok(number)
    }

    fn decimal(&mut self, value: Decimal) -> Result<(), CreateDatabaseError> {
        let mut digits = [0; NUMBER_BYTES];
        let mut at = NUMBER_BYTES;
        let mut magnitude = value.coefficient().unsigned_abs();
        let fraction = usize::from(value.scale());
        for _ in 0..fraction {
            at -= 1;
            digits[at] = b'0' + (magnitude % RADIX) as u8;
            magnitude /= RADIX;
        }
        if fraction != 0 {
            at -= 1;
            digits[at] = b'.';
        }
        loop {
            at -= 1;
            digits[at] = b'0' + (magnitude % RADIX) as u8;
            magnitude /= RADIX;
            if magnitude == 0 {
                break;
            }
        }
        if value.coefficient() < 0 {
            at -= 1;
            digits[at] = b'-';
        }
        let text = std::str::from_utf8(&digits[at..]).map_err(|_| CreateDatabaseError::Carrier)?;
        self.write_str(text)
            .map_err(|_| CreateDatabaseError::Carrier)
    }

    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
}

#[cfg(test)]
mod tests;
