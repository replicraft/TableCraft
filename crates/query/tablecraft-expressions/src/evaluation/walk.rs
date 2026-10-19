use super::Evaluator;
use crate::{
    EvalError,
    comparison::{self, Comparison},
    logic::Truth,
    numeric,
    predicates::{self, Membership},
};
use tablecraft_core::{
    limits::Resource,
    values::{Double, Value},
};
use tablecraft_sql::select::Expression as E;

const MAX_NUMBER_LITERAL_BYTES: usize = 128;

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
            E::GuidLiteral(text) => crate::guid::parse_literal(text).map(Value::Guid),
            E::DateLiteral(text) => crate::datetime::parse_literal(text).map(Value::DateTime),
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
                self.apply_unary(*operator, value)
            }
            E::Binary {
                left,
                operator,
                right,
            } => {
                let left = self.node(left, depth + 1)?;
                let right = self.node(right, depth + 1)?;
                self.apply_binary(*operator, &left, &right)
            }
            E::IsNull {
                expression,
                negated,
            } => {
                let value = self.node(expression, depth + 1)?;
                Ok(Value::Boolean(matches!(value, Value::Null) != *negated))
            }
            E::Between {
                expression,
                first_bound,
                second_bound,
                negated,
            } => {
                let value = self.node(expression, depth + 1)?;
                let first = self.node(first_bound, depth + 1)?;
                let second = self.node(second_bound, depth + 1)?;
                predicates::between(&value, &first, &second, *negated, |op, a, b| {
                    self.predicate_compare(op, a, b)
                })
                .map(Truth::into_value)
            }
            E::InList {
                expression,
                values,
                negated,
            } => {
                if values.is_empty() {
                    return Err(EvalError::UnsupportedExpression);
                }
                let value = self.node(expression, depth + 1)?;
                let mut membership = Membership::new();
                for item in values {
                    let candidate = self.node(item, depth + 1)?;
                    membership.consider(&value, &candidate, |a, b| {
                        self.predicate_compare(Comparison::Equal, a, b)
                    })?;
                }
                Ok(membership.finish(*negated).into_value())
            }
            E::Like {
                expression,
                pattern,
                negated,
            } => {
                let value = self.node(expression, depth + 1)?;
                let pattern = self.node(pattern, depth + 1)?;
                self.apply_like(&value, &pattern, *negated)
            }
            E::Call { .. }
            | E::Identity
            | E::Member { .. }
            | E::Subquery(_)
            | E::Exists(_)
            | E::InQuery { .. }
            | E::Quantified { .. } => Err(EvalError::UnsupportedExpression),
        }
    }
    fn predicate_compare(
        &mut self,
        operator: Comparison,
        left: &Value,
        right: &Value,
    ) -> Result<Truth, EvalError> {
        if self.cancelled.is_some_and(|cancelled| cancelled()) {
            return Err(EvalError::Cancelled);
        }
        self.budget.charge(Resource::Steps, 1)?;
        comparison::compare(operator, left, right, self.collation)
    }
}
fn number(text: &str) -> Result<Value, EvalError> {
    if text.len() > MAX_NUMBER_LITERAL_BYTES
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
