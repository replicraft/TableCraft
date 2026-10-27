//! Provisional complete French legacy ordering, separate from index payloads.
use super::{NativeSortOrder, comparison, key};
use crate::{InspectError, header::check};
use std::cmp::Ordering;
use tablecraft_core::limits::ResourceBudget;

/// Compare complete decoded Strings under exact French legacy 1036/0/0.
///
/// This engineering model reverses the complete secondary weight stream, ignores
/// trailing ASCII spaces and retains input beyond the native index prefix/digest.
/// Long unprintable positions use the existing widened engineering representation.
/// It is not native Access SQL/NLS qualification, LIKE, search or index pushdown.
/// Both operands are encoded eagerly under the same cumulative allowance.
pub fn compare_french_legacy_text(
    order: NativeSortOrder,
    left: &str,
    right: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ordering, InspectError> {
    check(budget, cancelled)?;
    qualify(order)?;
    let mut work = key::Work { budget, cancelled };
    let left = text_key(left, &mut work)?;
    let right = text_key(right, &mut work)?;
    comparison::compare_keys(&left, &right, &mut work)
}

/// Compare complete raw UTF-16 Strings under the provisional French model.
/// Isolated surrogates retain their measured weights. No physical truncation or
/// digest is used. This has the same padding, eager work and qualification scope
/// as compare_french_legacy_text; matching capabilities remain separate.
pub fn compare_french_legacy_utf16(
    order: NativeSortOrder,
    left: &[u16],
    right: &[u16],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ordering, InspectError> {
    check(budget, cancelled)?;
    qualify(order)?;
    let mut work = key::Work { budget, cancelled };
    let left = units_key(left, &mut work)?;
    let right = units_key(right, &mut work)?;
    comparison::compare_keys(&left, &right, &mut work)
}

fn qualify(order: NativeSortOrder) -> Result<(), InspectError> {
    if !order.is_french_legacy() {
        return Err(InspectError::Unsupported);
    }
    Ok(())
}

fn text_key(text: &str, work: &mut key::Work<'_>) -> Result<key::ComparisonKey, InspectError> {
    let mut units = work.vector::<u16>(text.len())?;
    for unit in text.encode_utf16() {
        work.tick()?;
        units.push(unit);
    }
    units_key(&units, work)
}

fn units_key(units: &[u16], work: &mut key::Work<'_>) -> Result<key::ComparisonKey, InspectError> {
    let units = comparison::trim_spaces(units, work)?;
    key::encode_full_french(units, work)
}
