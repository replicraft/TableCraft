use super::support::*;
use std::cmp::Ordering;
use tablecraft_expressions::{
    EvalError,
    comparison::{Collation, LikeProfile, StringComparisonProfile},
};

pub(super) struct Units(pub(super) bool);

impl Collation for Units {
    fn compare(&self, _: &str, _: &str) -> Result<Ordering, EvalError> {
        Err(EvalError::UnsupportedExpression)
    }

    fn string_comparison_profile(&self) -> StringComparisonProfile {
        StringComparisonProfile::Utf16
    }

    fn compare_utf16(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        self.compare_like_units(left, right)
    }

    fn like_profile(&self) -> LikeProfile {
        LikeProfile::Utf16Weights
    }

    fn compare_like_units(&self, left: &[u16], right: &[u16]) -> Result<Ordering, EvalError> {
        let fold = |unit: &u16| {
            if self.0 && (65..=90).contains(unit) {
                unit + 32
            } else {
                *unit
            }
        };
        Ok(left.iter().map(fold).cmp(right.iter().map(fold)))
    }

    fn is_like_unit(&self, units: &[u16]) -> Result<bool, EvalError> {
        Ok(units.len() == 1)
    }
}

pub(super) struct RawResolver;

impl CollationResolver for RawResolver {
    fn resolve(&self, id: TextCollationId) -> Result<&dyn Collation, QueryError> {
        match id.host_key() {
            1 => Ok(&Units(true)),
            2 => Ok(&Units(false)),
            _ => Err(EvalError::CollationRequired.into()),
        }
    }
}
