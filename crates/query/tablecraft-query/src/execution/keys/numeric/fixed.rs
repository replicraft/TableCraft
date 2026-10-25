//! Exact fixed/binary key ordering without decimal or floating-point rounding.
use std::cmp::Ordering;
use tablecraft_core::values::{Currency, Decimal, Value};
use tablecraft_expressions::EvalError;

const FRACTION_BITS: u32 = f64::MANTISSA_DIGITS - 1;
const EXPONENT_BITS: u32 = u64::BITS - 1 - FRACTION_BITS;
const EXPONENT_BIAS: i32 = (1 << (EXPONENT_BITS - 1)) - 1;
const FRACTION_MASK: u64 = (1 << FRACTION_BITS) - 1;
const EXPONENT_MASK: u64 = (1 << EXPONENT_BITS) - 1;
const DECIMAL_ODD_FACTOR: u128 = 5;

// m * 5^scale fits u128: binary64 has 53 significand bits and scale <= 28.
// Fail compilation if a future stored Decimal envelope invalidates this bound.
const _: () = assert!(
    f64::MANTISSA_DIGITS + DECIMAL_ODD_FACTOR.pow(Decimal::MAX_SCALE as u32).ilog2() < u128::BITS
);
const _: () = assert!(Decimal::COEFFICIENT_BITS <= u128::BITS);
const _: () = assert!(Currency::SCALE <= Decimal::MAX_SCALE);

pub(in crate::execution::keys) struct FixedBinary {
    coefficient: i128,
    scale: u8,
    binary: f64,
    reversed: bool,
}

pub(in crate::execution::keys) fn mixed_fixed_pair(
    left: &Value,
    right: &Value,
) -> Option<FixedBinary> {
    let (coefficient, scale, binary, reversed) =
        if let (Some((coefficient, scale)), Some(binary)) = (fixed(left), binary(right)) {
            (coefficient, scale, binary, false)
        } else if let (Some(binary), Some((coefficient, scale))) = (binary(left), fixed(right)) {
            (coefficient, scale, binary, true)
        } else {
            return None;
        };

    Some(FixedBinary {
        coefficient,
        scale,
        binary,
        reversed,
    })
}

impl FixedBinary {
    pub(in crate::execution::keys) fn compare(&self) -> Result<Ordering, EvalError> {
        if !self.binary.is_finite() {
            return Err(EvalError::NonFinite);
        }

        let bits = self.binary.to_bits();
        let exponent = ((bits >> FRACTION_BITS) & EXPONENT_MASK) as i32;
        let fraction = bits & FRACTION_MASK;
        let (significand, power) = if exponent == 0 {
            (fraction, 1 - EXPONENT_BIAS - FRACTION_BITS as i32)
        } else {
            (
                fraction | (1 << FRACTION_BITS),
                exponent - EXPONENT_BIAS - FRACTION_BITS as i32,
            )
        };

        let binary_sign = if significand == 0 {
            0
        } else if self.binary.is_sign_negative() {
            -1
        } else {
            1
        };

        let sign_order = self.coefficient.signum().cmp(&binary_sign);
        let order = if sign_order != Ordering::Equal {
            sign_order
        } else if significand == 0 {
            Ordering::Equal
        } else {
            // c / 10^s versus m * 2^e reduces to c versus m * 5^s * 2^(e+s).
            // Compare bit lengths before shifting, so subnormal and huge binary
            // exponents never require an oversized integer or allocation.
            let right = u128::from(significand)
                .checked_mul(DECIMAL_ODD_FACTOR.pow(u32::from(self.scale)))
                .ok_or(EvalError::Overflow)?;
            let order = scaled_order(
                self.coefficient.unsigned_abs(),
                right,
                power + i32::from(self.scale),
            )?;
            if self.coefficient < 0 {
                order.reverse()
            } else {
                order
            }
        };

        Ok(if self.reversed {
            order.reverse()
        } else {
            order
        })
    }
}

fn scaled_order(left: u128, right: u128, power: i32) -> Result<Ordering, EvalError> {
    let left_bits = (u128::BITS - left.leading_zeros()) as i32;
    let right_bits = (u128::BITS - right.leading_zeros()) as i32 + power;
    let order = left_bits.cmp(&right_bits);
    if order != Ordering::Equal {
        return Ok(order);
    }

    if power >= 0 {
        let shifted = right.checked_shl(power as u32).ok_or(EvalError::Overflow)?;
        Ok(left.cmp(&shifted))
    } else {
        let shifted = left
            .checked_shl((-power) as u32)
            .ok_or(EvalError::Overflow)?;
        Ok(shifted.cmp(&right))
    }
}

fn fixed(value: &Value) -> Option<(i128, u8)> {
    let (coefficient, scale) = match value {
        Value::Boolean(value) => (-i128::from(*value), 0),
        Value::Byte(value) => (i128::from(*value), 0),
        Value::Int16(value) => (i128::from(*value), 0),
        Value::Int32(value) => (i128::from(*value), 0),
        Value::Int64(value) => (i128::from(*value), 0),
        Value::Currency(value) => (i128::from(value.scaled()), Currency::SCALE),
        Value::Decimal(value) => (value.coefficient(), value.scale()),
        _ => return None,
    };
    Some((coefficient, scale))
}

fn binary(value: &Value) -> Option<f64> {
    match value {
        Value::Single(value) => Some(f64::from(value.to_float())),
        Value::Double(value) => Some(value.to_float()),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
