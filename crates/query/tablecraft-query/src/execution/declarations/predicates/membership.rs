//! IN-list comparison categories; every written candidate still evaluates.
use super::super::combined;
use crate::{
    QueryError,
    execution::{Environment, Runtime, cell::CellValue},
    plan::Expr,
};
use tablecraft_expressions::{comparison::Comparison, predicates::Membership};

impl Runtime<'_, '_> {
    pub(in crate::execution) fn declared_membership(
        &mut self,
        expression: &Expr,
        list: &[Expr],
        negated: bool,
        env: Environment<'_>,
    ) -> Result<CellValue, QueryError> {
        let value = self.expression_operand(expression, env)?;
        let mut declaration = value.declaration;
        let mut membership = Membership::new();
        for candidate in list {
            let item = self.expression_operand(candidate, env)?;
            declaration = combined(declaration, item.declaration);
            let policy = self.expression_pair_policy(expression, candidate, &value, &item)?;
            membership.consider(&value, &item, |left, right| {
                self.compare_expression_predicate_policy(Comparison::Equal, left, right, policy)
            })?;
        }
        Ok(
            CellValue::scalar(membership.finish(negated).into_value())
                .with_declaration(declaration),
        )
    }
}
