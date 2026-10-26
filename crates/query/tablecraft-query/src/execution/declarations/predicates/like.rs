//! LIKE result categories and actual String provenance after operand coercion.
use super::super::combined;
use crate::{
    QueryError,
    execution::{Environment, Runtime, cell::CellValue},
    plan::Expr,
    source::TextPolicy,
};

impl Runtime<'_, '_> {
    pub(in crate::execution) fn declared_like(
        &mut self,
        left: &Expr,
        right: &Expr,
        negated: bool,
        env: Environment<'_>,
    ) -> Result<CellValue, QueryError> {
        let left_cell = self.retained_expression_operand(left, env)?;
        let right_cell = self.retained_expression_operand(right, env)?;
        let declaration = combined(left_cell.declaration, right_cell.declaration);
        let prepared = self
            .evaluator()
            .prepare_expression_like(&left_cell.value, &right_cell.value)?;

        // NULL needs neither conversion nor a comparison provider. Both SQL
        // operand expressions have already run eagerly above.
        if prepared.is_null() {
            let value = self.evaluator().apply_prepared_like(&prepared, negated)?;
            return Ok(CellValue::scalar(value).with_declaration(declaration));
        }

        // Implicit String conversion constructs fresh default lineage. Only
        // operands that were already Strings contribute field policies.
        let left_policy = self.like_operand_policy(left, &left_cell, prepared.value_converted())?;
        let right_policy =
            self.like_operand_policy(right, &right_cell, prepared.pattern_converted())?;
        let collation = self.string_comparator(left_policy.merge(right_policy))?;
        let evaluator = self.evaluator();
        let mut evaluator = if let Some(collation) = collation {
            evaluator.with_collation(collation)
        } else {
            evaluator
        };
        let value = evaluator.apply_prepared_like(&prepared, negated)?;
        Ok(CellValue::scalar(value).with_declaration(declaration))
    }

    fn like_operand_policy(
        &mut self,
        expression: &Expr,
        cell: &CellValue,
        converted: bool,
    ) -> Result<TextPolicy, QueryError> {
        if converted {
            return Ok(TextPolicy::Default);
        }

        let fallback = if cell.policy_complete {
            TextPolicy::Default
        } else {
            self.text_policy(expression)?
        };
        Ok(fallback.merge(cell.policy))
    }
}
