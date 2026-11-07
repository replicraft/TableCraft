//! Independent projection from the audited numeric token and unary sign.
use super::property;
use crate::{EditError, header::check};
use tablecraft_core::{
    limits::ResourceBudget,
    values::{Double, Single, Value},
};
use tablecraft_schema::ScalarType;
use tablecraft_sql::select::Expression;

pub(super) fn derive(
    expression: &Expression,
    kind: ScalarType,
    negative: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, EditError> {
    check(budget, cancelled)?;
    if kind == ScalarType::Single {
        let magnitude: f32 = match expression {
            Expression::Number(token) => token.parse().map_err(|_| property())?,
            Expression::Integer(integer) => *integer as f32,
            _ => return Err(property()),
        };
        let result = if negative { -magnitude } else { magnitude };
        if result.is_finite() {
            return Ok(Value::Single(Single::from_bits(result.to_bits())));
        }
    } else if kind == ScalarType::Double {
        let magnitude: f64 = match expression {
            Expression::Number(token) => token.parse().map_err(|_| property())?,
            Expression::Integer(integer) => *integer as f64,
            _ => return Err(property()),
        };
        let result = if negative { -magnitude } else { magnitude };
        if result.is_finite() {
            return Ok(Value::Double(Double::from_bits(result.to_bits())));
        }
    }
    Err(property())
}
