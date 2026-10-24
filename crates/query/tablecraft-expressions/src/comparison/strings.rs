//! Explicit complete-unit comparison, separate from LIKE literal semantics.
use super::{Collation, Comparison, StringInput, from_ordering, string_input::check};
use crate::{EvalError, logic::Truth};
use tablecraft_core::limits::ResourceBudget;

/// The String input representation explicitly accepted by a comparison provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StringComparisonProfile {
    /// Existing decoded Unicode input; raw Strings require strict publication.
    Unicode,
    /// Complete UTF-16 units, including isolated surrogates. The provider owns
    /// ordering, padding, case and locale semantics; no binary order is inferred.
    Utf16,
}

/// Compare non-NULL String inputs with an explicitly selected raw-unit provider.
/// Checked unit buffers and callback guards use the supplied shared budget.
/// Odd byte lengths, scalar-only providers and callback failures are refused;
/// this service never substitutes LIKE weights or retries with another provider.
pub fn compare_utf16_strings(
    operator: Comparison,
    left: StringInput<'_>,
    right: StringInput<'_>,
    collation: &dyn Collation,
    budget: &mut ResourceBudget,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Result<Truth, EvalError> {
    check(budget, cancelled)?;
    if collation.string_comparison_profile() != StringComparisonProfile::Utf16 {
        return Err(EvalError::UnsupportedExpression);
    }

    let left = left.units(budget, cancelled)?;
    let right = right.units(budget, cancelled)?;
    check(budget, cancelled)?;
    let result = collation.compare_utf16(&left, &right)?;
    check(budget, cancelled)?;
    Ok(from_ordering(operator, result))
}

#[cfg(test)]
mod tests;
