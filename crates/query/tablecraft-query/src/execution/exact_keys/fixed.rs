//! Fingerprint views preserve SQL fixed-point equality without changing cells.
use super::check;
use crate::QueryError;
use tablecraft_core::{limits::ResourceBudget, values::Decimal};

/// Remove only fractional trailing zeros. Core bounds make signed division safe;
/// no scale increase, coefficient multiplication or float conversion is needed.
pub(super) fn canonical_decimal(
    value: Decimal,
    budget: &mut ResourceBudget,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Result<(i128, u8), QueryError> {
    let mut coefficient = value.coefficient();
    let mut scale = value.scale();
    if coefficient == 0 {
        return Ok((0, 0));
    }
    while scale > 0 {
        check(budget, cancelled)?;
        if coefficient % 10 != 0 {
            break;
        }
        coefficient /= 10;
        scale -= 1;
    }
    Ok((coefficient, scale))
}

#[cfg(test)]
mod tests;
