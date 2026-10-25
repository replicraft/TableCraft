//! Restrict only wholly safe predicates; partial AND extraction could hide eager
//! evaluation failures on rows discarded by a source. Residual filters remain.
mod currency;
mod text;
use super::Runtime;
use crate::{QueryError, plan::Expr, source::ScanConstraint};
use std::ops::Bound::{self, Excluded, Included, Unbounded};
use tablecraft_core::values::{Currency, Guid, Value};
use tablecraft_expressions::functions::conversions::Conversion;
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::select::{BinaryOperator as B, Expression, UnaryOperator as U};

impl Runtime<'_, '_> {
    pub(super) fn scan_constraint(
        &mut self,
        expr: &Expr,
        columns: &[Column],
        projection: &[usize],
        allow_text: bool,
    ) -> Result<Option<ScanConstraint>, QueryError> {
        self.scan_constraint_inner(expr, columns, projection, allow_text)
    }

    fn scan_constraint_inner(
        &mut self,
        expr: &Expr,
        columns: &[Column],
        projection: &[usize],
        allow_text: bool,
    ) -> Result<Option<ScanConstraint>, QueryError> {
        self.tick()?;
        Ok(match expr {
            Expr::Binary(B::And, left, right) => {
                let left = self.scan_constraint_inner(left, columns, projection, false)?;
                let right = self.scan_constraint_inner(right, columns, projection, false)?;
                left.zip(right)
                    .and_then(|(left, right)| intersect(left, right))
            }
            Expr::IsNull(field, negated) => {
                field_column(field, columns, projection).and_then(|column| {
                    if columns[column].data_type() == ScalarType::Boolean {
                        return None;
                    }
                    if *negated {
                        if columns[column].data_type() == ScalarType::Currency {
                            Some(ScanConstraint::Currency {
                                column,
                                lower: Unbounded,
                                upper: Unbounded,
                            })
                        } else {
                            range(column, columns[column].data_type(), Unbounded, Unbounded)
                        }
                    } else {
                        Some(ScanConstraint::Null { column })
                    }
                })
            }
            Expr::Binary(
                op @ (B::Equal | B::Less | B::LessOrEqual | B::Greater | B::GreaterOrEqual),
                left,
                right,
            ) => {
                let (column, constant, reversed) =
                    if let Some(column) = field_column(left, columns, projection) {
                        (column, right.as_ref(), false)
                    } else if let Some(column) = field_column(right, columns, projection) {
                        (column, left.as_ref(), true)
                    } else {
                        return Ok(None);
                    };
                if columns[column].data_type() == ScalarType::Boolean {
                    return Ok(match (op, constant) {
                        (B::Equal, Expr::Literal(Expression::Boolean(value))) => {
                            Some(ScanConstraint::Boolean {
                                column,
                                value: *value,
                            })
                        }
                        _ => None,
                    });
                }
                if columns[column].data_type() == ScalarType::Text {
                    return if allow_text && *op == B::Equal {
                        let field = if reversed {
                            right.as_ref()
                        } else {
                            left.as_ref()
                        };
                        self.text_equality_constraint(column, field, constant)
                    } else {
                        Ok(None)
                    };
                }
                if columns[column].data_type() == ScalarType::Guid {
                    return if *op == B::Equal {
                        Ok(self
                            .guid_constant(constant)?
                            .map(|value| ScanConstraint::Guid { column, value }))
                    } else {
                        Ok(None)
                    };
                }
                if columns[column].data_type() == ScalarType::Currency {
                    return Ok(self.currency_constant(constant)?.map(|value| {
                        let (lower, upper) = comparison_bounds(*op, reversed, value);
                        ScanConstraint::Currency {
                            column,
                            lower,
                            upper,
                        }
                    }));
                }
                self.integer_constant(constant, true)?.and_then(|value| {
                    let (lower, upper) = comparison_bounds(*op, reversed, value);
                    range(column, columns[column].data_type(), lower, upper)
                })
            }
            _ => None,
        })
    }

    fn guid_constant(&mut self, expr: &Expr) -> Result<Option<Guid>, QueryError> {
        self.tick()?;
        let value = match expr {
            Expr::Literal(literal @ Expression::GuidLiteral(_)) => {
                self.evaluator().evaluate(literal)?
            }
            Expr::Parameter(index) => match self.parameters.get(*index).map(|p| p.value) {
                Some(Value::Guid(value)) => return Ok(Some(*value)),
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };
        Ok(match value {
            Value::Guid(value) => Some(value),
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
            Expr::NumericLiteral(Value::Int16(value)) => Some(i64::from(*value)),
            Expr::NumericLiteral(Value::Int32(value)) => Some(i64::from(*value)),
            Expr::Parameter(index) if parameter => {
                match self.parameters.get(*index).map(|p| p.value) {
                    Some(Value::Byte(value)) => Some(i64::from(*value)),
                    Some(Value::Int16(value)) => Some(i64::from(*value)),
                    Some(Value::Int32(value)) => Some(i64::from(*value)),
                    Some(Value::Int64(value)) => Some(*value),
                    _ => None,
                }
            }
            Expr::Convert(
                conversion @ (Conversion::Byte | Conversion::Integer | Conversion::Long),
                inner,
            ) => self
                .integer_constant(inner, false)?
                .and_then(|value| match conversion {
                    Conversion::Byte => u8::try_from(value).ok().map(i64::from),
                    Conversion::Integer => i16::try_from(value).ok().map(i64::from),
                    Conversion::Long => i32::try_from(value).ok().map(i64::from),
                    _ => unreachable!("matched integral conversion"),
                }),
            Expr::Unary(U::Plus, inner) => self.integer_constant(inner, false)?,
            Expr::Unary(U::Minus, inner) => self
                .integer_constant(inner, false)?
                .and_then(i64::checked_neg),
            _ => None,
        })
    }
}

fn field_column(expr: &Expr, columns: &[Column], projection: &[usize]) -> Option<usize> {
    if let Expr::Field {
        depth: 0, index, ..
    } = expr
        && matches!(
            columns.get(*projection.get(*index)?)?.data_type(),
            ScalarType::Boolean
                | ScalarType::Byte
                | ScalarType::Int16
                | ScalarType::Int32
                | ScalarType::Int64
                | ScalarType::Guid
                | ScalarType::Currency
                | ScalarType::Text
                | ScalarType::Binary
        )
    {
        Some(*projection.get(*index)?)
    } else {
        None
    }
}

fn comparison_bounds<T: Copy>(operator: B, reversed: bool, value: T) -> (Bound<T>, Bound<T>) {
    match (operator, reversed) {
        (B::Equal, _) => (Included(value), Included(value)),
        (B::Less, false) | (B::Greater, true) => (Unbounded, Excluded(value)),
        (B::LessOrEqual, false) | (B::GreaterOrEqual, true) => (Unbounded, Included(value)),
        (B::Greater, false) | (B::Less, true) => (Excluded(value), Unbounded),
        (B::GreaterOrEqual, false) | (B::LessOrEqual, true) => (Included(value), Unbounded),
        _ => unreachable!("matched comparison operator"),
    }
}

fn range(
    column: usize,
    kind: ScalarType,
    lower: Bound<i64>,
    upper: Bound<i64>,
) -> Option<ScanConstraint> {
    match kind {
        ScalarType::Byte => Some(ScanConstraint::Byte {
            column,
            lower: narrow_bound(lower)?,
            upper: narrow_bound(upper)?,
        }),
        ScalarType::Int16 => Some(ScanConstraint::Int16 {
            column,
            lower: narrow_bound(lower)?,
            upper: narrow_bound(upper)?,
        }),
        ScalarType::Int32 => Some(ScanConstraint::Int32 {
            column,
            lower: narrow_bound(lower)?,
            upper: narrow_bound(upper)?,
        }),
        ScalarType::Int64 => Some(ScanConstraint::Int64 {
            column,
            lower,
            upper,
        }),
        _ => None,
    }
}

fn narrow_bound<T: TryFrom<i64>>(bound: Bound<i64>) -> Option<Bound<T>> {
    Some(match bound {
        Included(value) => Included(T::try_from(value).ok()?),
        Excluded(value) => Excluded(T::try_from(value).ok()?),
        Unbounded => Unbounded,
    })
}

fn intersect(left: ScanConstraint, right: ScanConstraint) -> Option<ScanConstraint> {
    let column = left.column();
    if column != right.column() {
        return None;
    }
    Some(match (left, right) {
        (ScanConstraint::Null { .. }, ScanConstraint::Null { .. }) => left,
        (ScanConstraint::Boolean { value: a, .. }, ScanConstraint::Boolean { value: b, .. })
            if a == b =>
        {
            left
        }
        (
            ScanConstraint::Byte {
                lower: a, upper: b, ..
            },
            ScanConstraint::Byte {
                lower: c, upper: d, ..
            },
        ) => ScanConstraint::Byte {
            column,
            lower: tighter_lower(a, c),
            upper: tighter_upper(b, d),
        },
        (ScanConstraint::Null { .. }, ScanConstraint::Byte { .. })
        | (ScanConstraint::Byte { .. }, ScanConstraint::Null { .. }) => ScanConstraint::Byte {
            column,
            lower: Included(0),
            upper: Excluded(0),
        },

        (
            ScanConstraint::Int16 {
                lower: a, upper: b, ..
            },
            ScanConstraint::Int16 {
                lower: c, upper: d, ..
            },
        ) => ScanConstraint::Int16 {
            column,
            lower: tighter_lower(a, c),
            upper: tighter_upper(b, d),
        },
        (ScanConstraint::Null { .. }, ScanConstraint::Int16 { .. })
        | (ScanConstraint::Int16 { .. }, ScanConstraint::Null { .. }) => ScanConstraint::Int16 {
            column,
            lower: Included(0),
            upper: Excluded(0),
        },

        (ScanConstraint::Guid { value: a, .. }, ScanConstraint::Guid { value: b, .. })
            if a == b =>
        {
            left
        }
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
        (
            ScanConstraint::Currency {
                lower: a, upper: b, ..
            },
            ScanConstraint::Currency {
                lower: c, upper: d, ..
            },
        ) => ScanConstraint::Currency {
            column,
            lower: tighter_lower(a.map(Currency::scaled), c.map(Currency::scaled))
                .map(Currency::from_scaled),
            upper: tighter_upper(b.map(Currency::scaled), d.map(Currency::scaled))
                .map(Currency::from_scaled),
        },
        (ScanConstraint::Null { .. }, ScanConstraint::Currency { .. })
        | (ScanConstraint::Currency { .. }, ScanConstraint::Null { .. }) => {
            ScanConstraint::Currency {
                column,
                lower: Included(Currency::from_scaled(0)),
                upper: Excluded(Currency::from_scaled(0)),
            }
        }
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
