//! Bounded exact aggregate totals beyond the stored 96-bit Decimal envelope.
use crate::QueryError;
use num_bigint::{BigInt, Sign};
use num_integer::Integer;
use num_rational::BigRational;
use num_traits::{Signed, ToPrimitive};
use std::cmp::Ordering;
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{Decimal, Value},
};
use tablecraft_expressions::{
    EvalError,
    numeric::{self, Arithmetic},
};

const DECIMAL_RADIX: i128 = 10;
const ALIGNMENT_BITS: u32 = (DECIMAL_RADIX as u128)
    .pow(Decimal::MAX_SCALE as u32)
    .ilog2()
    + 1;
const MAX_TOTAL_BYTES: u64 =
    (Decimal::COEFFICIENT_BITS + ALIGNMENT_BITS + i64::BITS).div_ceil(u8::BITS) as u64;
const INTEGER_BUFFER_COPIES: u64 = 8;

pub(super) struct Total {
    coefficient: Coefficient,
    scale: u8,
}

enum Coefficient {
    Small(i128),
    Large(BigInt),
}

fn shifted(value: i128, from: u8, to: u8) -> Option<i128> {
    value.checked_mul(DECIMAL_RADIX.pow(u32::from(to - from)))
}

/// A narrow library sum can be retained only if rescaling/addition lose no digits.
pub(super) fn narrow_sum(left: Decimal, right: Decimal) -> bool {
    let mut scale = left.scale().max(right.scale());
    let sum = shifted(left.coefficient(), left.scale(), scale)
        .zip(shifted(right.coefficient(), right.scale(), scale))
        .and_then(|(a, b)| a.checked_add(b));
    let Some(mut coefficient) = sum else {
        return false;
    };
    while coefficient.unsigned_abs() > Decimal::MAX_COEFFICIENT as u128
        && scale > 0
        && coefficient % DECIMAL_RADIX == 0
    {
        coefficient /= DECIMAL_RADIX;
        scale -= 1;
    }
    coefficient.unsigned_abs() <= Decimal::MAX_COEFFICIENT as u128
}

fn check(
    budget: &mut ResourceBudget,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Result<(), QueryError> {
    if cancelled.is_some_and(|callback| callback()) {
        return Err(QueryError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}

fn buffers(budget: &mut ResourceBudget) -> Result<(), QueryError> {
    budget.charge(
        Resource::Bytes,
        (MAX_TOTAL_BYTES + size_of::<BigInt>() as u64) * INTEGER_BUFFER_COPIES,
    )?;
    Ok(())
}

impl Total {
    pub(super) fn new(value: Decimal) -> Self {
        Self {
            coefficient: Coefficient::Small(value.coefficient()),
            scale: value.scale(),
        }
    }

    fn integer(&self) -> BigInt {
        match &self.coefficient {
            Coefficient::Small(value) => BigInt::from(*value),
            Coefficient::Large(value) => value.clone(),
        }
    }

    pub(super) fn push(
        &mut self,
        value: Decimal,
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<(), QueryError> {
        check(budget, cancelled)?;
        let scale = self.scale.max(value.scale());
        if let Coefficient::Small(coefficient) = &self.coefficient
            && let Some(sum) = shifted(*coefficient, self.scale, scale)
                .zip(shifted(value.coefficient(), value.scale(), scale))
                .and_then(|(a, b)| a.checked_add(b))
        {
            self.coefficient = Coefficient::Small(sum);
        } else {
            buffers(budget)?;
            let left =
                self.integer() * BigInt::from(DECIMAL_RADIX).pow(u32::from(scale - self.scale));
            let right = BigInt::from(value.coefficient())
                * BigInt::from(DECIMAL_RADIX).pow(u32::from(scale - value.scale()));
            self.coefficient = Coefficient::Large(left + right);
        }
        self.scale = scale;
        Ok(())
    }

    fn narrow(&self) -> Option<Decimal> {
        let mut coefficient = match &self.coefficient {
            Coefficient::Small(value) => *value,
            Coefficient::Large(value) => value.to_i128()?,
        };
        let mut scale = self.scale;
        while coefficient.unsigned_abs() > Decimal::MAX_COEFFICIENT as u128
            && scale > 0
            && coefficient % DECIMAL_RADIX == 0
        {
            coefficient /= DECIMAL_RADIX;
            scale -= 1;
        }
        Decimal::new(coefficient, scale).ok()
    }

    /// Final SUM conversion preserves a fitting exact coefficient/scale. Wider
    /// totals use the same highest-fitting-scale half-even rule as Decimal means.
    pub(super) fn sum(
        &self,
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<Value, QueryError> {
        check(budget, cancelled)?;
        if let Some(value) = self.narrow() {
            return Ok(Value::Decimal(value));
        }
        self.decimal(1, budget, cancelled)
    }

    pub(super) fn double(
        &self,
        count: i64,
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<f64, QueryError> {
        check(budget, cancelled)?;
        if count <= 0 {
            return Err(QueryError::Source);
        }
        buffers(budget)?;
        let denominator =
            BigInt::from(count) * BigInt::from(DECIMAL_RADIX).pow(u32::from(self.scale));
        let result = BigRational::new_raw(self.integer(), denominator)
            .to_f64()
            .ok_or(EvalError::Overflow)?;
        if result.is_finite() {
            Ok(result)
        } else {
            Err(EvalError::Overflow.into())
        }
    }

    pub(super) fn decimal(
        &self,
        count: i64,
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<Value, QueryError> {
        check(budget, cancelled)?;
        if count <= 0 {
            return Err(QueryError::Source);
        }
        if let Some(total) = self.narrow() {
            return Ok(numeric::promoted_arithmetic(
                Arithmetic::Divide,
                &Value::Decimal(total),
                &Value::Int64(count),
            )?);
        }
        for scale in (0..=Decimal::MAX_SCALE).rev() {
            check(budget, cancelled)?;
            buffers(budget)?;
            let mut numerator = self.integer();
            let mut denominator = BigInt::from(count);
            if scale >= self.scale {
                numerator *= BigInt::from(DECIMAL_RADIX).pow(u32::from(scale - self.scale));
            } else {
                denominator *= BigInt::from(DECIMAL_RADIX).pow(u32::from(self.scale - scale));
            }
            let (mut coefficient, remainder) = numerator.div_rem(&denominator);
            let halfway = (remainder.abs() << 1_usize).cmp(&denominator);
            if halfway == Ordering::Greater || (halfway == Ordering::Equal && coefficient.is_odd())
            {
                if numerator.sign() == Sign::Minus {
                    coefficient -= 1;
                } else {
                    coefficient += 1;
                }
            }
            if let Some(coefficient) = coefficient.to_i128()
                && let Ok(value) = Decimal::new(coefficient, scale)
            {
                return Ok(Value::Decimal(value));
            }
        }
        Err(EvalError::Overflow.into())
    }
}
