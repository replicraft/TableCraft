//! Restrict only wholly safe predicates; partial AND extraction could hide eager
//! evaluation failures on rows discarded by a source. Residual filters remain.
use super::Runtime;
use crate::{QueryError, plan::Expr, source::ScanConstraint};
use std::ops::Bound::{self, Excluded, Included, Unbounded};
use tablecraft_core::values::Value;
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::select::{BinaryOperator as B, Expression, UnaryOperator as U};

impl Runtime<'_> {
    pub(super) fn scan_constraint(
        &mut self,
        expr: &Expr,
        columns: &[Column],
    ) -> Result<Option<ScanConstraint>, QueryError> {
        self.tick()?;
        Ok(match expr {
            Expr::Binary(B::And, left, right) => {
                let left = self.scan_constraint(left, columns)?;
                let right = self.scan_constraint(right, columns)?;
                left.zip(right)
                    .and_then(|(left, right)| intersect(left, right))
            }
            Expr::IsNull(field, negated) => field_column(field, columns).and_then(|column| {
                if *negated {
                    range(column, columns[column].data_type(), Unbounded, Unbounded)
                } else {
                    Some(ScanConstraint::Null { column })
                }
            }),
            Expr::Binary(
                op @ (B::Equal | B::Less | B::LessOrEqual | B::Greater | B::GreaterOrEqual),
                left,
                right,
            ) => {
                let (column, value, reversed) = if let Some(column) = field_column(left, columns) {
                    (Some(column), self.integer_constant(right, true)?, false)
                } else {
                    (
                        field_column(right, columns),
                        self.integer_constant(left, true)?,
                        true,
                    )
                };
                column.zip(value).and_then(|(column, value)| {
                    let (lower, upper) = match (op, reversed) {
                        (B::Equal, _) => (Included(value), Included(value)),
                        (B::Less, false) | (B::Greater, true) => (Unbounded, Excluded(value)),
                        (B::LessOrEqual, false) | (B::GreaterOrEqual, true) => {
                            (Unbounded, Included(value))
                        }
                        (B::Greater, false) | (B::Less, true) => (Excluded(value), Unbounded),
                        (B::GreaterOrEqual, false) | (B::LessOrEqual, true) => {
                            (Included(value), Unbounded)
                        }
                        _ => unreachable!("matched comparison operator"),
                    };
                    range(column, columns[column].data_type(), lower, upper)
                })
            }
            _ => None,
        })
    }
    // Parse integers without evaluating arbitrary expressions or introducing
    // new errors on empty input. Unsupported/overflowing constants decline hints.
    fn integer_constant(
        &mut self,
        expr: &Expr,
        parameter: bool,
    ) -> Result<Option<i64>, QueryError> {
        self.tick()?;
        Ok(match expr {
            Expr::Literal(Expression::Integer(value)) => Some(*value),
            Expr::Parameter(index) if parameter => match self.parameters.get(*index) {
                Some(Value::Int32(value)) => Some(i64::from(*value)),
                Some(Value::Int64(value)) => Some(*value),
                _ => None,
            },
            Expr::Unary(U::Plus, inner) => self.integer_constant(inner, false)?,
            Expr::Unary(U::Minus, inner) => self
                .integer_constant(inner, false)?
                .and_then(i64::checked_neg),
            _ => None,
        })
    }
}

fn field_column(expr: &Expr, columns: &[Column]) -> Option<usize> {
    if let Expr::Field { depth: 0, index } = expr
        && matches!(
            columns.get(*index)?.data_type(),
            ScalarType::Int32 | ScalarType::Int64
        )
    {
        Some(*index)
    } else {
        None
    }
}

fn range(
    column: usize,
    kind: ScalarType,
    lower: Bound<i64>,
    upper: Bound<i64>,
) -> Option<ScanConstraint> {
    let narrow = |bound| {
        Some(match bound {
            Included(value) => Included(i32::try_from(value).ok()?),
            Excluded(value) => Excluded(i32::try_from(value).ok()?),
            Unbounded => Unbounded,
        })
    };
    match kind {
        ScalarType::Int32 => Some(ScanConstraint::Int32 {
            column,
            lower: narrow(lower)?,
            upper: narrow(upper)?,
        }),
        ScalarType::Int64 => Some(ScanConstraint::Int64 {
            column,
            lower,
            upper,
        }),
        _ => None,
    }
}

fn intersect(left: ScanConstraint, right: ScanConstraint) -> Option<ScanConstraint> {
    let column = left.column();
    if column != right.column() {
        return None;
    }
    Some(match (left, right) {
        (ScanConstraint::Null { .. }, ScanConstraint::Null { .. }) => left,
        (
            ScanConstraint::Int32 {
                lower: a, upper: b, ..
            },
            ScanConstraint::Int32 {
                lower: c, upper: d, ..
            },
        ) => ScanConstraint::Int32 {
            column,
            lower: tighter_lower(a, c),
            upper: tighter_upper(b, d),
        },
        (
            ScanConstraint::Int64 {
                lower: a, upper: b, ..
            },
            ScanConstraint::Int64 {
                lower: c, upper: d, ..
            },
        ) => ScanConstraint::Int64 {
            column,
            lower: tighter_lower(a, c),
            upper: tighter_upper(b, d),
        },
        (ScanConstraint::Null { .. }, ScanConstraint::Int32 { .. })
        | (ScanConstraint::Int32 { .. }, ScanConstraint::Null { .. }) => ScanConstraint::Int32 {
            column,
            lower: Included(0),
            upper: Excluded(0),
        },
        (ScanConstraint::Null { .. }, ScanConstraint::Int64 { .. })
        | (ScanConstraint::Int64 { .. }, ScanConstraint::Null { .. }) => ScanConstraint::Int64 {
            column,
            lower: Included(0),
            upper: Excluded(0),
        },
        _ => return None,
    })
}
fn tighter_lower<T: Copy + Ord>(a: Bound<T>, b: Bound<T>) -> Bound<T> {
    match (a, b) {
        (Unbounded, other) | (other, Unbounded) => other,
        (Included(x) | Excluded(x), Included(y) | Excluded(y)) => {
            if x > y || x == y && matches!(a, Excluded(_)) {
                a
            } else {
                b
            }
        }
    }
}
fn tighter_upper<T: Copy + Ord>(a: Bound<T>, b: Bound<T>) -> Bound<T> {
    match (a, b) {
        (Unbounded, other) | (other, Unbounded) => other,
        (Included(x) | Excluded(x), Included(y) | Excluded(y)) => {
            if x < y || x == y && matches!(a, Excluded(_)) {
                a
            } else {
                b
            }
        }
    }
}
