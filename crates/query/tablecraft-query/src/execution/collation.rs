//! Explicit per-operation routing; no source-specific identity decoding.
use super::Runtime;
use crate::{QueryError, plan::Expr, source::TextPolicy};
use tablecraft_core::values::Value;
use tablecraft_expressions::comparison::{self, Collation, Comparison};
use tablecraft_expressions::logic::Truth;

impl<'a> Runtime<'a, '_> {
    pub(super) fn text_policy(&mut self, expr: &Expr) -> Result<TextPolicy, QueryError> {
        if self.options.collation_resolver.is_none() {
            return Ok(TextPolicy::Default);
        }
        crate::plan::text_policy::expression(expr, &mut || self.tick(), 1)
    }
    pub(super) fn text_pair_policy(
        &mut self,
        left: &Expr,
        right: &Expr,
        a: &Value,
        b: &Value,
    ) -> Result<TextPolicy, QueryError> {
        if !matches!((a, b), (Value::Text(_), Value::Text(_))) {
            return Ok(TextPolicy::Default);
        }
        Ok(self.text_policy(left)?.merge(self.text_policy(right)?))
    }
    pub(super) fn expression_policies(
        &mut self,
        expressions: &[Expr],
    ) -> Result<Vec<TextPolicy>, QueryError> {
        if self.options.collation_resolver.is_none() {
            return Ok(Vec::new());
        }
        self.budget
            .charge_allocation(expressions.len() as u64, size_of::<TextPolicy>() as u64)?;
        let mut policies = Vec::new();
        policies
            .try_reserve_exact(expressions.len())
            .map_err(|_| QueryError::Allocation)?;
        for expr in expressions {
            policies.push(self.text_policy(expr)?);
        }
        Ok(policies)
    }
    pub(super) fn comparator(
        &mut self,
        policy: TextPolicy,
        a: &Value,
        b: &Value,
    ) -> Result<Option<&'a dyn Collation>, QueryError> {
        let Some(resolver) = self.options.collation_resolver else {
            return Ok(self.options.collation);
        };
        if !matches!((a, b), (Value::Text(_), Value::Text(_))) {
            return Ok(self.options.collation);
        }
        let key = match policy {
            TextPolicy::Default => return Ok(self.options.collation),
            TextPolicy::Column(key) => key,
            TextPolicy::Conflict => return Err(QueryError::CollationConflict),
        };
        let _span = tracing::debug_span!("query.collation.resolve").entered();
        self.tick()?;
        let comparer = resolver.resolve(key)?;
        self.tick()?;
        Ok(Some(comparer))
    }
    pub(super) fn compare_policy(
        &mut self,
        op: Comparison,
        a: &Value,
        b: &Value,
        policy: TextPolicy,
    ) -> Result<Truth, QueryError> {
        self.tick()?;
        let collation = self.comparator(policy, a, b)?;
        let result = comparison::compare(op, a, b, collation)?;
        self.tick()?;
        Ok(result)
    }
}
