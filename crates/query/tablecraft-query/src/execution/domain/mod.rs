//! Explicit read-only domain evaluation within the selected host source namespace.
use super::{Environment, Runtime};
use crate::{
    QueryError,
    binding::bind_domain,
    plan::{DomainFunction, Expr},
    profile::MAX_NAMED_DEPTH,
};
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;
use tablecraft_expressions::functions::conversions::errors::into_scalar;

const MAX_ARGUMENTS: usize = 3;
impl Runtime<'_> {
    pub(in crate::execution) fn domain(
        &mut self,
        kind: DomainFunction,
        arguments: &[Expr],
        env: Environment<'_>,
    ) -> Result<Value, QueryError> {
        self.tick()?;
        let mut values: [Value; MAX_ARGUMENTS] = std::array::from_fn(|_| Value::Null);
        for (expr, value) in arguments.iter().zip(&mut values) {
            *value = self.eval(expr, env)?;
        }
        let expression = text(&values[0])?;
        let domain = text(&values[1])?;
        let criteria = if arguments.len() == MAX_ARGUMENTS {
            Some(text(&values[2])?)
        } else {
            None
        };
        if self.domain_depth == MAX_NAMED_DEPTH {
            return Err(QueryError::StructureLimit);
        }
        self.domain_depth += 1;
        let result = (|| {
            let _span = tracing::debug_span!("query.domain.read").entered();
            let Some(plan) = bind_domain(
                self.source,
                kind,
                expression,
                domain,
                criteria,
                self.budget,
                self.options.cancelled,
            )?
            else {
                return Ok(Value::Null);
            };
            let mut rows = self.read(&plan, &[])?;
            self.tick()?;
            if rows.len() != 1 || rows[0].len() != 1 {
                return Err(QueryError::Width);
            }
            rows.pop()
                .and_then(|mut row| row.pop())
                .ok_or(QueryError::Width)
                .and_then(|value| into_scalar(value).map_err(QueryError::from))
        })();
        self.domain_depth -= 1;
        result
    }
}

fn text(value: &Value) -> Result<&str, EvalError> {
    match value {
        Value::Text(s) => Ok(s.as_str()),
        Value::Null => Err(EvalError::InvalidNullConversion),
        _ => Err(EvalError::TypeMismatch),
    }
}
