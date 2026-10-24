//! Translate fixed-point samples exactly before their first binary64 rounding.
use crate::QueryError;
use num_bigint::BigInt;
use std::fmt::{self, Write};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Decimal, Value},
};
use tablecraft_expressions::{EvalError, functions::conversions::Conversion};

const RADIX: i128 = 10;
const ALIGNMENT_BITS: u32 = (RADIX as u128).pow(Decimal::MAX_SCALE as u32).ilog2() + 1;
// Aligning opposite signed 96-bit coefficients can require one additional bit.
const DIFFERENCE_BITS: u32 = Decimal::COEFFICIENT_BITS + ALIGNMENT_BITS + 1;
const INTEGER_BYTES: u64 = DIFFERENCE_BITS.div_ceil(u8::BITS) as u64;
// Covers aligned operands, arithmetic temporaries and bounded integer-display scratch.
const INTEGER_SCRATCH_COPIES: u64 = 12;
const COEFFICIENT_DIGITS: usize = Decimal::MAX_PRECISION as usize + Decimal::MAX_SCALE as usize + 1;
const SCALE_DIGITS: usize = Decimal::MAX_SCALE.ilog10() as usize + 1;
// Coefficient sign, exponent marker/sign and the scale's decimal digits.
const SPELLING_BYTES: usize = COEFFICIENT_DIGITS + 1 + 2 + SCALE_DIGITS;

pub(super) fn sample(value: &Value) -> Result<Option<Decimal>, QueryError> {
    if matches!(value, Value::Single(_) | Value::Double(_)) {
        return Ok(None);
    }
    let Value::Decimal(value) = Conversion::Decimal.apply(value)? else {
        return Err(QueryError::Source);
    };
    Ok(Some(value))
}

pub(super) fn offset(
    value: Decimal,
    anchor: Decimal,
    budget: &mut ResourceBudget,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Result<f64, QueryError> {
    check(budget, cancelled)?;
    let scale = value.scale().max(anchor.scale());
    let value_factor = RADIX.pow(u32::from(scale - value.scale()));
    let anchor_factor = RADIX.pow(u32::from(scale - anchor.scale()));
    let narrow = value
        .coefficient()
        .checked_mul(value_factor)
        .zip(anchor.coefficient().checked_mul(anchor_factor))
        .and_then(|(value, anchor)| value.checked_sub(anchor));
    let mut spelling = Spelling::default();
    if let Some(difference) = narrow {
        write!(&mut spelling, "{difference}e-{scale}").map_err(|_| QueryError::Source)?;
    } else {
        // BigInt is only needed for unequal scales whose exact aligned
        // coefficients exceed i128. Charge a bounded operation/display allowance
        // before constructing any wide integer or formatter scratch allocation.
        budget.charge(
            Resource::Bytes,
            (INTEGER_BYTES + size_of::<BigInt>() as u64 + SPELLING_BYTES as u64)
                * INTEGER_SCRATCH_COPIES,
        )?;
        let difference = BigInt::from(value.coefficient()) * BigInt::from(value_factor)
            - BigInt::from(anchor.coefficient()) * BigInt::from(anchor_factor);
        check(budget, cancelled)?;
        write!(&mut spelling, "{difference}e-{scale}").map_err(|_| QueryError::Source)?;
    }
    check(budget, cancelled)?;
    let value = spelling
        .as_str()?
        .parse::<f64>()
        .map_err(|_| QueryError::Source)?;
    if !value.is_finite() {
        return Err(EvalError::Overflow.into());
    }
    Ok(value)
}

fn check(
    budget: &mut ResourceBudget,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Result<(), QueryError> {
    if cancelled.is_some_and(|cancelled| cancelled()) {
        return Err(QueryError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}

struct Spelling {
    bytes: [u8; SPELLING_BYTES],
    length: usize,
}
impl Default for Spelling {
    fn default() -> Self {
        Self {
            bytes: [0; SPELLING_BYTES],
            length: 0,
        }
    }
}
impl Spelling {
    fn as_str(&self) -> Result<&str, QueryError> {
        std::str::from_utf8(&self.bytes[..self.length]).map_err(|_| QueryError::Source)
    }
}
impl Write for Spelling {
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

#[cfg(test)]
mod tests;
