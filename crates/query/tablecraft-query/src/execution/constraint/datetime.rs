//! Date hints retain captured temporal values without invoking conversions.
use super::{Expr, Runtime, ScanConstraint};
use crate::QueryError;
use std::ops::Bound;
use tablecraft_core::values::{LegacyDateTime, Value};

impl Runtime<'_, '_> {
    pub(super) fn datetime_constant(
        &mut self,
        expression: &Expr,
    ) -> Result<Option<LegacyDateTime>, QueryError> {
        self.tick()?;
        Ok(match expression {
            Expr::DateLiteral(_, value) => Some(*value),
            Expr::Parameter(index) => match self.parameters.get(*index).map(|value| value.value) {
                Some(Value::DateTime(value)) => Some(*value),
                _ => None,
            },
            _ => None,
        })
    }
}

pub(super) fn intersection(
    column: usize,
    lower: Bound<LegacyDateTime>,
    upper: Bound<LegacyDateTime>,
    other_lower: Bound<LegacyDateTime>,
    other_upper: Bound<LegacyDateTime>,
) -> ScanConstraint {
    ScanConstraint::DateTime {
        column,
        lower: tighter(lower, other_lower, true),
        upper: tighter(upper, other_upper, false),
    }
}

pub(super) fn empty(column: usize) -> Option<ScanConstraint> {
    // A guaranteed-valid constant supplies a temporal empty interval. Declining
    // the optional hint would preserve the original predicate if that invariant
    // ever changed; no source value or expression is converted here.
    let epoch = LegacyDateTime::from_bits(0).ok()?;
    Some(ScanConstraint::DateTime {
        column,
        lower: Bound::Included(epoch),
        upper: Bound::Excluded(epoch),
    })
}

fn tighter(
    left: Bound<LegacyDateTime>,
    right: Bound<LegacyDateTime>,
    lower: bool,
) -> Bound<LegacyDateTime> {
    match (left, right) {
        (Bound::Unbounded, other) | (other, Bound::Unbounded) => other,
        (Bound::Included(a) | Bound::Excluded(a), Bound::Included(b) | Bound::Excluded(b)) => {
            let order = a.temporal_cmp(b);
            if (lower && order.is_gt())
                || (!lower && order.is_lt())
                || (order.is_eq() && matches!(left, Bound::Excluded(_)))
            {
                left
            } else {
                right
            }
        }
    }
}
