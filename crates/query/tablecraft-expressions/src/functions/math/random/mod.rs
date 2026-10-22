//! Explicit Basic-compatible Rnd sequence; no entropy, clock or process-global state.
use crate::{EvalError, functions::conversions::Conversion};
use std::cell::Cell;
use tablecraft_core::values::{Single, Value};

const STATE_BITS: u32 = 24;
const STATE_MODULUS: u32 = 1 << STATE_BITS;
const MULTIPLIER: u32 = 0x43fd43fd;
const INCREMENT: u32 = 0x00c39ec3;

/// Caller-owned, sequential Basic Rnd state. It is deliberately not Sync: hosts
/// must specify sequence ownership rather than race multiple query evaluations.
/// Raw seeds initialize state directly; they do not implement Randomize.
pub struct RndState {
    state: Cell<u32>,
}

impl RndState {
    /// Largest raw state accepted by the 24-bit generator.
    pub const MAX_SEED: u32 = STATE_MODULUS - 1;
    /// Reviewed Basic initial state, independent of clocks or entropy sources.
    pub const DEFAULT_SEED: u32 = 0x00050000;

    /// Initialize explicit raw state; values outside the state width are errors.
    pub fn new(seed: u32) -> Result<Self, EvalError> {
        if seed > Self::MAX_SEED {
            return Err(EvalError::Domain);
        }
        Ok(Self {
            state: Cell::new(seed),
        })
    }

    /// Apply an omitted/positive, zero or negative numeric argument. Validation
    /// completes before advancing state; callers meter/check cancellation first.
    pub fn apply(&self, number: Option<&Value>) -> Result<Value, EvalError> {
        let number = match number {
            Some(number) => {
                let Value::Single(number) = Conversion::Single.apply(number)? else {
                    return Err(EvalError::TypeMismatch);
                };
                number.to_float()
            }
            None => 1.0,
        };
        let mut seed = self.state.get();
        if number < 0.0 {
            let bits = number.to_bits();
            seed = bits.wrapping_add(bits >> STATE_BITS) & Self::MAX_SEED;
        }
        if number != 0.0 {
            seed = seed.wrapping_mul(MULTIPLIER).wrapping_add(INCREMENT) & Self::MAX_SEED;
        }
        self.state.set(seed);
        Ok(Value::Single(Single::from_float(
            seed as f32 / STATE_MODULUS as f32,
        )))
    }
}

impl Default for RndState {
    fn default() -> Self {
        Self {
            state: Cell::new(Self::DEFAULT_SEED),
        }
    }
}

#[cfg(test)]
mod tests;
