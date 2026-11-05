//! Native common-filter pattern policy, rebound to each inspected database.
use super::{Criterion, FieldFilterOperator, FieldFilterPatternMode, FilterError, Predicate};
use crate::{
    Error,
    controls::{SCOPE, check},
};
use tablecraft_ace::{DatabaseQueryMode, NativeReadSource, inspect_database_properties};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_expressions::{
    EvalError,
    comparison::{Collation, LikeProfile},
    predicates::{LikeCase, LikeMatch, LikeMode, LikeString, PreparedLikeSearch},
};

pub(super) const PATTERN_BYTES: u64 = 64 * 1024 * 1024;
pub(super) const PATTERN_STEPS: u64 = 10_000_000;

pub(super) fn mode<R: NativeReadSource>(
    source: &mut R,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<LikeMode, Error> {
    let stored = match inspect_database_properties(source, SCOPE, work, cancelled)? {
        Some(properties) => properties.query_mode(work, cancelled)?,
        None => DatabaseQueryMode::Absent,
    };
    match stored {
        DatabaseQueryMode::Absent | DatabaseQueryMode::Ansi89 => Ok(LikeMode::Ansi89),
        DatabaseQueryMode::Ansi92 => Ok(LikeMode::Ansi92),
        DatabaseQueryMode::Unknown => Err(FilterError::UnknownPatternMode.into()),
    }
}

pub(super) fn mode_if_needed<R: NativeReadSource>(
    source: &mut R,
    predicates: &[Criterion],
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<LikeMode>, Error> {
    check(cancelled)?;
    work.charge(Resource::Steps, predicates.len() as u64)?;
    if predicates
        .iter()
        .any(|criterion| matches!(criterion.predicate, Predicate::Pattern { .. }))
    {
        mode(source, work, cancelled).map(Some)
    } else {
        Ok(None)
    }
}

pub(super) fn describe_mode(mode: LikeMode) -> FieldFilterPatternMode {
    match mode {
        LikeMode::Ansi89 => FieldFilterPatternMode::Ansi89,
        LikeMode::Ansi92 => FieldFilterPatternMode::Ansi92,
    }
}

pub(super) fn qualify(policy: &dyn Collation) -> Result<(), Error> {
    if policy.like_profile() != LikeProfile::Utf16Weights {
        return Err(EvalError::CollationRequired.into());
    }
    Ok(())
}

pub(super) fn compile(
    criterion: &Criterion,
    mode: Option<LikeMode>,
    work: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<PreparedLikeSearch>, Error> {
    check(cancelled)?;
    let Predicate::Pattern { operator, input } = &criterion.predicate else {
        return Ok(None);
    };
    let mode = mode.ok_or(FilterError::UnknownPatternMode)?;
    let wildcard = match mode {
        LikeMode::Ansi89 => '*',
        LikeMode::Ansi92 => '%',
    };
    let prefix = matches!(
        operator,
        FieldFilterOperator::EndsWith
            | FieldFilterOperator::NotEndsWith
            | FieldFilterOperator::Contains
            | FieldFilterOperator::NotContains
    );
    let suffix = matches!(
        operator,
        FieldFilterOperator::BeginsWith
            | FieldFilterOperator::NotBeginsWith
            | FieldFilterOperator::Contains
            | FieldFilterOperator::NotContains
    );
    let length = input
        .len()
        .checked_add(usize::from(prefix) + usize::from(suffix))
        .ok_or(FilterError::InputLimit)?;
    work.charge_allocation(length as u64, 1)?;
    work.charge(Resource::Steps, length as u64)?;
    let mut pattern = String::new();
    pattern.try_reserve_exact(length)?;
    if prefix {
        pattern.push(wildcard);
    }
    pattern.push_str(input);
    if suffix {
        pattern.push(wildcard);
    }
    Ok(Some(PreparedLikeSearch::new(
        LikeString::Text(&pattern),
        mode,
        LikeMatch::Whole,
        LikeCase::Database,
        work,
        Some(cancelled),
    )?))
}
