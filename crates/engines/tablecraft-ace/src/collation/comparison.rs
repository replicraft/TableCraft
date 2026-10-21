//! Provisional full-string General weight ordering, distinct from index payloads.
use super::{GeneralSortFamily, NativeSortOrder, key};
use crate::{InspectError, header::check};
use std::cmp::Ordering;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::comparison::LikeSpanState;

const SPACE_UNIT: u16 = b' ' as u16;

/// Compare complete decoded text using the reviewed General weight model.
///
/// This is a provisional comparison policy, not native Access SQL qualification.
/// It retains all UTF-16 units, ignores trailing ASCII spaces, and widens internal
/// unprintable positions instead of using native truncated/digested index keys.
/// Unknown locale/variant/version descriptors fail without an ordinal fallback.
/// The caller owns cumulative resource/cancellation limits. No database setting,
/// query policy, LIKE mode or automatic index selection is activated here.
pub fn compare_general_text(
    order: NativeSortOrder,
    left: &str,
    right: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ordering, InspectError> {
    check(budget, cancelled)?;
    let family = order.general_family().ok_or(InspectError::Unsupported)?;
    let mut work = key::Work { budget, cancelled };
    // Validate/encode both inputs even when a primary prefix already differs.
    let left = full_key(family, left, &mut work)?;
    let right = full_key(family, right, &mut work)?;
    compare_keys(&left, &right, &mut work)
}

/// Compare complete raw General LIKE literals, including isolated surrogates.
/// Unlike SQL text comparison, trailing spaces retain their literal weights.
/// No physical prefix/digest or normalization supplies comparison results.
/// This is an explicit provisional model, not native Access LIKE qualification.
pub fn compare_general_like_units(
    order: NativeSortOrder,
    left: &[u16],
    right: &[u16],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ordering, InspectError> {
    check(budget, cancelled)?;
    let family = order.general_family().ok_or(InspectError::Unsupported)?;
    let mut work = key::Work { budget, cancelled };
    let left = key::encode_full(family, left, &mut work)?;
    let right = key::encode_full(family, right, &mut work)?;
    compare_keys(&left, &right, &mut work)
}

/// Match a raw literal or prove that its primary prefix cannot be extended into
/// that literal's complete key. This conservative General LIKE model bound uses
/// no truncated physical index key. Caller work/cancellation remain explicit.
pub fn general_like_literal_step(
    order: NativeSortOrder,
    candidate: &[u16],
    literal: &[u16],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<LikeSpanState, InspectError> {
    check(budget, cancelled)?;
    let family = order.general_family().ok_or(InspectError::Unsupported)?;
    let mut work = key::Work { budget, cancelled };
    let candidate = key::encode_full(family, candidate, &mut work)?;
    let literal = key::encode_full(family, literal, &mut work)?;
    if compare_keys(&candidate, &literal, &mut work)? == Ordering::Equal {
        return Ok(LikeSpanState::Match);
    }
    let prefix = &candidate.bytes[..candidate.primary_length];
    if prefix.len() > literal.bytes.len() {
        work.tick()?;
        return Ok(LikeSpanState::Impossible);
    }
    for (left, right) in prefix.iter().zip(&literal.bytes) {
        work.tick()?;
        if left != right {
            work.tick()?;
            return Ok(LikeSpanState::Impossible);
        }
    }
    work.tick()?;
    Ok(LikeSpanState::Possible)
}

pub(super) fn full_key(
    family: GeneralSortFamily,
    text: &str,
    work: &mut key::Work<'_>,
) -> Result<key::ComparisonKey, InspectError> {
    let mut units = work.vector::<u16>(text.len())?;
    for unit in text.encode_utf16() {
        work.tick()?;
        units.push(unit);
    }
    while units.last() == Some(&SPACE_UNIT) {
        work.tick()?;
        units.pop();
    }
    key::encode_full(family, &units, work)
}

pub(super) fn compare_keys(
    left: &key::ComparisonKey,
    right: &key::ComparisonKey,
    work: &mut key::Work<'_>,
) -> Result<Ordering, InspectError> {
    for (left, right) in left.bytes.iter().zip(&right.bytes) {
        work.tick()?;
        let order = left.cmp(right);
        if order != Ordering::Equal {
            work.tick()?;
            return Ok(order);
        }
    }
    work.tick()?;
    Ok(left.bytes.len().cmp(&right.bytes.len()))
}
