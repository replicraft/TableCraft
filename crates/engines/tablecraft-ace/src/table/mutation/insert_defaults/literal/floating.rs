//! Numeric AST literals only; preserve unary negative zero and IEEE width.
use super::unsupported;
use crate::{EditError, header::check};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, Single, Value},
};
use tablecraft_schema::ScalarType;
use tablecraft_sql::select::{Expression, UnaryOperator};

pub(super) fn resolve(
    expression: &Expression,
    kind: ScalarType,
    negative: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, EditError> {
    check(budget, cancelled)?;
    match expression {
        Expression::Unary {
            operator: UnaryOperator::Plus,
            expression,
        } => {
            return resolve(expression, kind, negative, budget, cancelled);
        }
        Expression::Unary {
            operator: UnaryOperator::Minus,
            expression,
        } => {
            return resolve(expression, kind, !negative, budget, cancelled);
        }
        _ => {}
    }
    match kind {
        ScalarType::Single => {
            let value = match expression {
                Expression::Integer(value) => *value as f32,
                Expression::Number(text) => text.parse::<f32>().map_err(|_| unsupported())?,
                _ => return Err(unsupported()),
            };
            let value = if negative { -value } else { value };
            if !value.is_finite() {
                return Err(unsupported());
            }
            Ok(Value::Single(Single::from_bits(value.to_bits())))
        }
        ScalarType::Double => {
            let value = match expression {
                Expression::Integer(value) => *value as f64,
                Expression::Number(text) => text.parse::<f64>().map_err(|_| unsupported())?,
                _ => return Err(unsupported()),
            };
            let value = if negative { -value } else { value };
            if !value.is_finite() {
                return Err(unsupported());
            }
            Ok(Value::Double(Double::from_bits(value.to_bits())))
        }
        _ => Err(unsupported()),
    }
}
