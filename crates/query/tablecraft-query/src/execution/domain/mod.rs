//! Explicit read-only domain evaluation within the selected host source namespace.
use super::{Environment, Runtime};
use crate::execution::cell::CellValue;
use crate::execution::cell::into_scalar;
use crate::{
    QueryError,
    binding::{DomainContext, bind_domain},
    plan::{DomainFunction, Expr},
    profile::MAX_NAMED_DEPTH,
};
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;

const MAX_ARGUMENTS: usize = 3;
impl Runtime<'_, '_> {
    pub(in crate::execution) fn domain(
        &mut self,
        kind: DomainFunction,
        arguments: &[Expr],
        env: Environment<'_>,
    ) -> Result<Value, QueryError> {
        let mut result = self.domain_expression(kind, arguments, env)?;
        result.value = self.evaluator().publish_expression_value(result.value)?;
        Ok(into_scalar(result)?)
    }

    pub(in crate::execution) fn domain_expression(
        &mut self,
        kind: DomainFunction,
        arguments: &[Expr],
        env: Environment<'_>,
    ) -> Result<CellValue, QueryError> {
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
            let plan = loop {
                let stage = bind_domain(
                    DomainContext {
                        binding_depth: self.options.binding_depth,
                        owner: self.owner,
                        external_sources: &self.discovery.external_sources.borrow(),
                        source: self.source,
                        materialized: &self.discovery.materialized.borrow(),
                        budget: self.budget,
                        cancelled: self.options.cancelled,
                        discover: self.discovery.enabled,
                        reference_year: self.options.reference_year,
                        declared_arithmetic: self.options.declared_integral,
                    },
                    kind,
                    expression,
                    domain,
                    criteria,
                )?;
                match stage {
                    Ok(Some(plan)) => break plan,
                    Ok(None) => return Ok(CellValue::scalar(Value::Null)),
                    Err(request) => {
                        self.discovery.remember_bindings(
                            request.external_sources,
                            self.budget,
                            self.options.cancelled,
                        )?;
                        self.discovery.discover_domain(
                            request.identity,
                            self.source,
                            self.options,
                            self.budget,
                            self.repeatable_dynamic,
                        )?;
                    }
                }
            };
            self.register_external_sources(&plan.external_sources)?;
            let mut rows = self.read(&plan.read, &[])?;
            self.tick()?;
            if rows.len() != 1 || rows[0].len() != 1 {
                return Err(QueryError::Width);
            }
            rows.pop()
                .and_then(|mut row| row.pop())
                .ok_or(QueryError::Width)
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
