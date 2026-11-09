//! Explicit provisional case-sensitive UTF-16 Find policy, separate from weights.
//! This is ordinal literal/class behavior, not qualified Windows NLS parity.
use super::budget::Budget;
use std::cmp::Ordering;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_expressions::{EvalError, comparison::LikeSpanState};

fn tick(work: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), EvalError> {
    if cancelled() {
        return Err(EvalError::Cancelled);
    }
    work.charge(Resource::Steps, 1)?;
    Ok(())
}

pub(super) fn compare(
    left: &[u16],
    right: &[u16],
    pool: &Budget<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ordering, EvalError> {
    let mut work = pool
        .try_borrow_mut()
        .map_err(|_| EvalError::CollationFailed)?;
    tick(&mut work, cancelled)?;
    for (left, right) in left.iter().zip(right) {
        tick(&mut work, cancelled)?;
        if left != right {
            return Ok(left.cmp(right));
        }
    }
    Ok(left.len().cmp(&right.len()))
}

pub(super) fn literal(
    candidate: &[u16],
    literal: &[u16],
    pool: &Budget<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<LikeSpanState, EvalError> {
    let mut work = pool
        .try_borrow_mut()
        .map_err(|_| EvalError::CollationFailed)?;
    tick(&mut work, cancelled)?;
    for (left, right) in candidate.iter().zip(literal) {
        tick(&mut work, cancelled)?;
        if left != right {
            return Ok(LikeSpanState::Impossible);
        }
    }
    Ok(match candidate.len().cmp(&literal.len()) {
        Ordering::Less => LikeSpanState::Possible,
        Ordering::Equal => LikeSpanState::Match,
        Ordering::Greater => LikeSpanState::Impossible,
    })
}

pub(super) fn unit(
    candidate: &[u16],
    pool: &Budget<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<LikeSpanState, EvalError> {
    let mut work = pool
        .try_borrow_mut()
        .map_err(|_| EvalError::CollationFailed)?;
    tick(&mut work, cancelled)?;
    Ok(match candidate.len() {
        0 => LikeSpanState::Possible,
        1 => LikeSpanState::Match,
        _ => LikeSpanState::Impossible,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FrenchLegacyCollation, GeneralCollation};
    use tablecraft_ace::NativeSortOrder;
    use tablecraft_expressions::{
        comparison::{Collation, StringInput, StringSearch},
        predicates::{LikeCase, LikeMatch, LikeMode, find_like_utf16},
    };

    #[test]
    fn explicit_case_profile_does_not_reuse_case_insensitive_native_weights() {
        let budget = || ResourceBudget::new(16 * 1024 * 1024, 0, 1_000_000);
        let general = GeneralCollation::new(
            NativeSortOrder::from_native_bytes([9, 4, 0, 1]),
            budget(),
            &|| false,
        )
        .unwrap();
        let french = FrenchLegacyCollation::new(
            NativeSortOrder::from_native_bytes([12, 4, 0, 0]),
            budget(),
            &|| false,
        )
        .unwrap();
        for policy in [&general as &dyn Collation, &french as &dyn Collation] {
            for (case, expected) in [(LikeCase::Database, 0..1), (LikeCase::Sensitive, 2..3)] {
                assert_eq!(
                    find_like_utf16(
                        StringInput::Text("A a"),
                        StringInput::Text("a"),
                        LikeMode::Ansi89,
                        LikeMatch::Any,
                        StringSearch::ForwardFrom(0),
                        case,
                        policy,
                        &mut budget(),
                        None,
                    )
                    .unwrap(),
                    Some(expected)
                );
            }
        }
    }
}
