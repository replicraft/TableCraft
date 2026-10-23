//! LIKE result categories around the existing explicit pattern/policy service.
use super::combined;
use crate::{
    QueryError,
    execution::{
        Environment, Runtime,
        cell::{CellValue, into_scalar},
    },
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
        let left_cell = self.expression_operand(left, env)?;
        let right_cell = self.expression_operand(right, env)?;
        let declaration = combined(left_cell.declaration, right_cell.declaration);
        let policy = if self.options.like_mode.is_some() {
            self.expression_pair_policy(left, right, &left_cell, &right_cell)?
                .merge(left_cell.policy)
                .merge(right_cell.policy)
        } else {
            TextPolicy::Default
        };
        let left = into_scalar(left_cell)?;
        let right = into_scalar(right_cell)?;
        let collation = self.comparator(policy, &left, &right)?;
        let evaluator = self.evaluator();
        let mut evaluator = if let Some(collation) = collation {
            evaluator.with_collation(collation)
        } else {
            evaluator
        };
        let value = evaluator.apply_like(&left, &right, negated)?;
        Ok(CellValue::scalar(value).with_declaration(declaration))
    }
}
