//! Untrimmed French pattern keys, separate from SQL/search padding.
use super::qualify;
use crate::{
    InspectError, NativeSortOrder,
    collation::{comparison, key, like_units::unit_set},
    header::check,
};
use std::cmp::Ordering;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::comparison::LikeSpanState;

#[cfg(test)]
mod artifact;
#[cfg(test)]
mod tests;

/// Compare complete French legacy LIKE spans without removing trailing spaces.
/// Raw UTF-16 units retain their weights, including isolated surrogates. This is
/// the provisional exact 1036/0/0 pattern policy, not native Access qualification.
/// Both full operands are encoded eagerly under the cumulative caller budget.
pub fn compare_french_legacy_like_units(
    order: NativeSortOrder,
    left: &[u16],
    right: &[u16],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ordering, InspectError> {
    check(budget, cancelled)?;
    qualify(order)?;
    let mut work = key::Work { budget, cancelled };
    let left = key::encode_full_french(left, &mut work)?;
    let right = key::encode_full_french(right, &mut work)?;
    comparison::compare_keys(&left, &right, &mut work)
}

/// Match a complete French literal or prove its primary prefix cannot match
/// by extension. Secondary reversal never supplies an impossibility proof.
/// Unlike SQL/search keys, trailing spaces are retained for both pattern spans.
pub fn french_legacy_like_literal_step(
    order: NativeSortOrder,
    candidate: &[u16],
    literal: &[u16],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<LikeSpanState, InspectError> {
    check(budget, cancelled)?;
    qualify(order)?;
    let mut work = key::Work { budget, cancelled };
    let candidate = key::encode_full_french(candidate, &mut work)?;
    let literal = key::encode_full_french(literal, &mut work)?;
    comparison::literal_step(&candidate, &literal, &mut work)
}

/// Whether a complete French pattern span equals some single raw unit's key.
/// The immutable dictionary contains the separately derived untrimmed full keys
/// of all 65,536 units. No per-call enumeration, index truncation, SQL padding or
/// runtime cache supplies element equivalence; caller controls remain cumulative.
pub fn is_french_legacy_like_unit(
    order: NativeSortOrder,
    candidate: &[u16],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, InspectError> {
    check(budget, cancelled)?;
    qualify(order)?;
    let mut work = key::Work { budget, cancelled };
    let key = key::encode_full_french(candidate, &mut work)?;
    unit_set::contains(&dictionary()?, &key, &mut work)
}

/// Recognize a complete French element or prove that its primary prefix cannot
/// extend into any dictionary key. Complete reversed-secondary keys decide
/// acceptance; only the append-only Legacy primary stream permits pruning.
pub fn french_legacy_like_unit_step(
    order: NativeSortOrder,
    candidate: &[u16],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<LikeSpanState, InspectError> {
    check(budget, cancelled)?;
    qualify(order)?;
    let mut work = key::Work { budget, cancelled };
    let key = key::encode_full_french(candidate, &mut work)?;
    unit_set::step(&dictionary()?, &key, &mut work)
}

fn dictionary() -> Result<fst::Set<&'static [u8]>, InspectError> {
    let bytes: &[u8] = include_bytes!("../../../data/general/like-french-legacy.fst");
    fst::Set::new(bytes).map_err(|_| InspectError::Malformed)
}
