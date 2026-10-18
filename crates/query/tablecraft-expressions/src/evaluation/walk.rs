use super::Evaluator;
use crate::{EvalError, logic::Truth, numeric};
use tablecraft_core::{
    limits::Resource,
    values::{Double, Value},
};
use tablecraft_sql::select::{Expression as E, UnaryOperator as U};

impl Evaluator<'_> {
    pub(super) fn node(&mut self, expression: &E, depth: usize) -> Result<Value, EvalError> {
        if self.cancelled.is_some_and(|cancelled| cancelled()) {
            return Err(EvalError::Cancelled);
        }
        if depth > self.limits.max_depth {
            return Err(EvalError::DepthLimit);
        }
        self.budget.charge(Resource::Steps, 1)?;
        match expression {
            E::Null => Ok(Value::Null),
            E::Boolean(value) => Ok(Value::Boolean(*value)),
            E::Integer(value) => Ok(Value::Int64(*value)),
            E::Text(text) => self.text(text),
            E::Number(text) => number(text),
            E::Field(field) => {
                let value = self.bindings.resolve(field)?;
                match value {
                    Value::Text(text) => self.text(text),
                    Value::Binary(_) => Err(EvalError::UnsupportedExpression),
                    Value::Single(_) | Value::Double(_) => {
                        numeric::float(value)?;
                        Ok(value.clone())
                    }
                    _ => Ok(value.clone()),
                }
            }
            E::Unary {
                operator,
                expression,
            } => {
                let value = self.node(expression, depth + 1)?;
                match operator {
                    U::Not => Truth::from_value(&value).map(|truth| truth.negate().into_value()),
                    U::Minus => numeric::negate(&value),
                    U::Plus => {
                        if matches!(value, Value::Null) || numeric::fixed(&value).is_some() {
                            Ok(value)
                        } else {
                            numeric::float(&value)?;
                            Ok(value)
                        }
                    }
                }
            }
            E::Binary {
                left,
                operator,
                right,
            } => {
                let left = self.node(left, depth + 1)?;
                let right = self.node(right, depth + 1)?;
                self.binary(*operator, &left, &right)
            }
            E::IsNull {
                expression,
                negated,
            } => {
                let value = self.node(expression, depth + 1)?;
                Ok(Value::Boolean(matches!(value, Value::Null) != *negated))
            }
            E::Call { .. }
            | E::DateLiteral(_)
            | E::GuidLiteral(_)
            | E::Identity
            | E::Like { .. }
            | E::Member { .. }
            | E::Subquery(_)
            | E::Exists(_)
            | E::InQuery { .. }
            | E::Quantified { .. }
            | E::Between { .. }
            | E::InList { .. } => Err(EvalError::UnsupportedExpression),
        }
    }
}
fn number(text: &str) -> Result<Value, EvalError> {
    if text.len() > 128
        || !text.contains(['.', 'e', 'E'])
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || b"+-.eE".contains(&b))
    {
        return Err(EvalError::UnsupportedLiteral);
    }
    let value: f64 = text.parse().map_err(|_| EvalError::UnsupportedLiteral)?;
    if !value.is_finite() {
        return Err(EvalError::Overflow);
    }
    let mantissa = text
        .split(['e', 'E'])
        .next()
        .ok_or(EvalError::UnsupportedLiteral)?;
    if value == 0.0 && mantissa.bytes().any(|b| (b'1'..=b'9').contains(&b)) {
        return Err(EvalError::PrecisionLoss);
    }
    Ok(Value::Double(Double::from_float(value)))
}
