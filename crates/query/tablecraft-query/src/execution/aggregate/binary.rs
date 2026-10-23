//! Exact IEEE-754 binary64 sums in bounded exponent bins; num owns division.
use crate::QueryError;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::ToPrimitive;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_expressions::EvalError;

const MAX_EXACT_FLOAT_COUNT: i64 = 1_i64 << f64::MANTISSA_DIGITS;
const FRACTION_BITS: u32 = f64::MANTISSA_DIGITS - 1;
const EXPONENT_BITS: u32 = u64::BITS - FRACTION_BITS - 1;
const EXPONENT_MASK: u64 = (1 << EXPONENT_BITS) - 1;
const FRACTION_MASK: u64 = (1 << FRACTION_BITS) - 1;
const EXPONENT_BINS: usize = EXPONENT_MASK as usize - 1;
const SUBNORMAL_EXPONENT: i32 = f64::MIN_EXP - f64::MANTISSA_DIGITS as i32;
const BINARY_UNIT_SHIFT: usize = (-SUBNORMAL_EXPONENT) as usize;
const MAX_SUM_BITS: usize = (f64::MAX_EXP - SUBNORMAL_EXPONENT) as usize + i64::BITS as usize;
const MAX_SUM_BYTES: u64 = MAX_SUM_BITS.div_ceil(u8::BITS as usize) as u64;
// Bounded term/total copies plus rational conversion's shifted quotient buffers.
const TERM_BUFFER_COPIES: u64 = 4;
const QUOTIENT_BUFFER_COPIES: u64 = 16;

pub(in crate::execution) struct Scratch {
    bins: Vec<i128>,
    seen: Vec<bool>,
    occupied: Vec<u16>,
    terms: u64,
    first: f64,
}
fn vector<T: Clone>(
    size: usize,
    fill: T,
    budget: &mut ResourceBudget,
) -> Result<Vec<T>, QueryError> {
    budget.charge_allocation(size as u64, size_of::<T>() as u64)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(size)
        .map_err(|_| QueryError::Allocation)?;
    values.resize(size, fill);
    Ok(values)
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
impl Scratch {
    pub(super) fn acquire(
        pool: &mut Vec<Self>,
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<Self, QueryError> {
        check(budget, cancelled)?;
        if let Some(mut scratch) = pool.pop() {
            for index in scratch.occupied.drain(..) {
                check(budget, cancelled)?;
                scratch.bins[usize::from(index)] = 0;
                scratch.seen[usize::from(index)] = false;
            }
            scratch.terms = 0;
            scratch.first = 0.0;
            return Ok(scratch);
        }
        let bins = vector(EXPONENT_BINS, 0_i128, budget)?;
        let seen = vector(EXPONENT_BINS, false, budget)?;
        let mut occupied = vector(EXPONENT_BINS, 0_u16, budget)?;
        occupied.clear();
        Ok(Self {
            bins,
            seen,
            occupied,
            terms: 0,
            first: 0.0,
        })
    }
    pub(super) fn release(
        self,
        pool: &mut Vec<Self>,
        budget: &mut ResourceBudget,
    ) -> Result<(), QueryError> {
        if pool.len() == pool.capacity() {
            budget.charge(Resource::Bytes, size_of::<Self>() as u64)?;
            pool.try_reserve_exact(1)
                .map_err(|_| QueryError::Allocation)?;
        }
        pool.push(self);
        Ok(())
    }
    pub(super) fn push(&mut self, value: f64) -> Result<(), EvalError> {
        if !value.is_finite() {
            return Err(EvalError::NonFinite);
        }
        let bits = value.to_bits();
        let exponent = ((bits >> FRACTION_BITS) & EXPONENT_MASK) as usize;
        let mut significand = bits & FRACTION_MASK;
        if exponent != 0 {
            significand |= 1 << FRACTION_BITS;
        }
        if significand == 0 {
            return Ok(());
        }
        self.terms = self.terms.checked_add(1).ok_or(EvalError::Overflow)?;
        if self.terms == 1 {
            self.first = value;
        }
        let index = exponent.saturating_sub(1);
        let coefficient = if value.is_sign_negative() {
            -i128::from(significand)
        } else {
            i128::from(significand)
        };
        self.bins[index] = self.bins[index]
            .checked_add(coefficient)
            .ok_or(EvalError::Overflow)?;
        if !self.seen[index] {
            self.seen[index] = true;
            self.occupied.push(index as u16);
        }
        Ok(())
    }
    pub(super) fn rounded(
        &self,
        count: i64,
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<f64, QueryError> {
        if count <= 0 {
            return Err(QueryError::Source);
        }
        check(budget, cancelled)?;
        // A single nonzero term needs only one correctly rounded hardware divide.
        // The count must itself be exactly representable; all query row ceilings
        // are smaller, but keep the helper's precondition explicit.
        if self.terms == 0 {
            return Ok(0.0);
        }
        if self.terms == 1 && count <= MAX_EXACT_FLOAT_COUNT {
            return Ok(self.first / count as f64);
        }
        let mut total = BigInt::from(0);
        for index in &self.occupied {
            check(budget, cancelled)?;
            let coefficient = self.bins[usize::from(*index)];
            if coefficient == 0 {
                continue;
            }
            budget.charge(Resource::Bytes, MAX_SUM_BYTES * TERM_BUFFER_COPIES)?;
            total += BigInt::from(coefficient) << usize::from(*index);
        }
        check(budget, cancelled)?;
        budget.charge(Resource::Bytes, MAX_SUM_BYTES * QUOTIENT_BUFFER_COPIES)?;
        let denominator = BigInt::from(count) << BINARY_UNIT_SHIFT;
        // The denominator is positive/nonzero. Only conversion is used: equality,
        // hashing and arithmetic on an unreduced Ratio are deliberately absent.
        let result = BigRational::new_raw(total, denominator)
            .to_f64()
            .ok_or(EvalError::Overflow)?;
        check(budget, cancelled)?;
        if result.is_finite() {
            Ok(result)
        } else {
            Err(EvalError::Overflow.into())
        }
    }
}
