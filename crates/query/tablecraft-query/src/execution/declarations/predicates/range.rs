//! Bounded inclusive-range comparisons and their composed declaration category.
use super::super::combined;
use crate::{
    QueryError,
    execution::{Environment, Runtime, cell::CellValue},
    plan::Expr,
};
use tablecraft_expressions::comparison::Comparison;

impl Runtime<'_, '_> {
    pub(in crate::execution) fn declared_range(
        &mut self,
        expression: &Expr,
        first: &Expr,
        second: &Expr,
        negated: bool,
        env: Environment<'_>,
    ) -> Result<CellValue, QueryError> {
        let value = self.expression_operand(expression, env)?;
        let left = self.expression_operand(first, env)?;
        let right = self.expression_operand(second, env)?;
        let declaration = combined(
            combined(value.declaration, left.declaration),
            right.declaration,
        );
        let first_policy = self.expression_pair_policy(expression, first, &value, &left)?;
        let second_policy = self.expression_pair_policy(expression, second, &value, &right)?;
        let forward = self
            .compare_expression_predicate_policy(
                Comparison::GreaterOrEqual,
                &value,
                &left,
                first_policy,
            )?
            .and(self.compare_expression_predicate_policy(
                Comparison::LessOrEqual,
                &value,
                &right,
                second_policy,
            )?);
        let reverse = self
            .compare_expression_predicate_policy(
                Comparison::GreaterOrEqual,
                &value,
                &right,
                second_policy,
            )?
            .and(self.compare_expression_predicate_policy(
                Comparison::LessOrEqual,
                &value,
                &left,
                first_policy,
            )?);
        let truth = forward.or(reverse);
        let truth = if negated { truth.negate() } else { truth };
        Ok(CellValue::scalar(truth.into_value()).with_declaration(declaration))
    }
}
