//! Currency hints admit typed parameters and closed numeric CCur literals only.
use super::super::Runtime;
use crate::{QueryError, plan::Expr};
use tablecraft_core::{
    limits::Resource,
    values::{Currency, Value},
};
use tablecraft_expressions::{EvalError, functions::conversions::Conversion};
use tablecraft_sql::select::{Expression, UnaryOperator};

impl Runtime<'_, '_> {
    pub(super) fn currency_constant(
        &mut self,
        expr: &Expr,
    ) -> Result<Option<Currency>, QueryError> {
        self.tick()?;
        if let Expr::Parameter(index) = expr {
            return Ok(match self.parameters.get(*index) {
                Some(Value::Currency(value)) => Some(*value),
                _ => None,
            });
        }
        let Expr::Convert(Conversion::Currency, inner) = expr else {
            return Ok(None);
        };
        let Some(value) = self.currency_numeric_literal(inner)? else {
            return Ok(None);
        };
        match Conversion::Currency.apply(&value) {
            Ok(Value::Currency(value)) => Ok(Some(value)),
            Ok(_) => Err(QueryError::Unsupported),
            Err(EvalError::Overflow | EvalError::PrecisionLoss | EvalError::UnsupportedLiteral) => {
                // Decline the hint: an invalid constant must not introduce a
                // semantic failure when the original query has no input rows.
                Ok(None)
            }
            Err(error) => Err(error.into()),
        }
    }

    fn currency_numeric_literal(&mut self, expr: &Expr) -> Result<Option<Value>, QueryError> {
        self.tick()?;
        let result = match expr {
            Expr::Literal(literal @ (Expression::Integer(_) | Expression::Number(_))) => {
                if let Expression::Number(text) = literal {
                    self.budget.charge(Resource::Steps, text.len() as u64)?;
                }
                self.evaluator().evaluate(literal)
            }
            Expr::Unary(operator @ (UnaryOperator::Plus | UnaryOperator::Minus), inner) => {
                let Some(value) = self.currency_numeric_literal(inner)? else {
                    return Ok(None);
                };
                self.evaluator().apply_unary(*operator, value)
            }
            _ => return Ok(None),
        };
        match result {
            Ok(value) => Ok(Some(value)),
            Err(EvalError::Overflow | EvalError::PrecisionLoss | EvalError::UnsupportedLiteral) => {
                Ok(None)
            }
            Err(error) => Err(error.into()),
        }
    }
}
