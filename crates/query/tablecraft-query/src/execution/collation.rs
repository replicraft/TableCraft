//! Explicit per-operation routing; no source-specific identity decoding.
mod provenance;

use super::Runtime;
use crate::execution::cell::CellValue;
use crate::{QueryError, plan::Expr, source::TextPolicy};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::comparison::{self, Collation, Comparison, variant_text_numeric};
use tablecraft_expressions::numeric::OperandDeclaration;
use tablecraft_expressions::{EvalError, logic::Truth};

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
    pub(super) fn expression_pair_policy(
        &mut self,
        left: &Expr,
        right: &Expr,
        a: &CellValue,
        b: &CellValue,
    ) -> Result<TextPolicy, QueryError> {
        match (&a.value, &b.value) {
            (ExpressionValue::Value(a), ExpressionValue::Value(b)) => {
                self.text_pair_policy(left, right, a, b)
            }
            _ => Ok(TextPolicy::Default),
        }
    }

    pub(super) fn compare_expression_predicate_policy(
        &mut self,
        op: Comparison,
        a: &CellValue,
        b: &CellValue,
        policy: TextPolicy,
    ) -> Result<Truth, QueryError> {
        let (left_policy, right_policy) = (a.policy, b.policy);
        let declared_variants = self.options.declared_integral
            && a.declaration == Some(OperandDeclaration::Variant)
            && b.declaration == Some(OperandDeclaration::Variant);
        if let (ExpressionValue::Value(a), ExpressionValue::Value(b)) = (&a.value, &b.value) {
            if declared_variants {
                self.tick()?;
                if let Some(result) = variant_text_numeric(
                    op,
                    a,
                    b,
                    OperandDeclaration::Variant,
                    OperandDeclaration::Variant,
                )? {
                    self.tick()?;
                    return Ok(result);
                }
            }
            let policy = policy.merge(left_policy).merge(right_policy);
            return self.compare_predicate_policy(op, a, b, policy);
        }
        self.tick()?;
        let result = self
            .evaluator()
            .compare_expression_values(op, a, b)
            .map_err(predicate_error)?;
        self.tick()?;
        Ok(result)
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
    pub(super) fn compare_predicate_policy(
        &mut self,
        op: Comparison,
        a: &Value,
        b: &Value,
        policy: TextPolicy,
    ) -> Result<Truth, QueryError> {
        self.tick()?;
        let collation = self.comparator(policy, a, b)?;
        let declared = self.options.declared_integral;
        let evaluator = self.evaluator();
        let mut evaluator = if let Some(collation) = collation {
            evaluator.with_collation(collation)
        } else {
            evaluator
        };
        let result = if declared {
            evaluator.compare_effective_numeric_values(op, a, b)
        } else {
            evaluator.compare_values(op, a, b)
        }
        .map_err(predicate_error)?;
        self.tick()?;
        Ok(result)
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

fn predicate_error(error: EvalError) -> QueryError {
    match error {
        EvalError::Cancelled => QueryError::Cancelled,
        EvalError::Limit(limit) => QueryError::Limit(limit),
        error => QueryError::Evaluation(error),
    }
}
