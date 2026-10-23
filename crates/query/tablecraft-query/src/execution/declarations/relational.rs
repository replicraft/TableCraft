//! Comparison categories; the existing policy-aware comparator still owns values.
use super::combined;
use crate::{
    QueryError,
    execution::{Runtime, cell::CellValue},
    plan::Expr,
};
use tablecraft_expressions::comparison::Comparison;
use tablecraft_sql::select::BinaryOperator;

pub(in crate::execution) fn operator(operator: BinaryOperator) -> Option<Comparison> {
    match operator {
        BinaryOperator::Equal => Some(Comparison::Equal),
        BinaryOperator::NotEqual => Some(Comparison::NotEqual),
        BinaryOperator::Less => Some(Comparison::Less),
        BinaryOperator::LessOrEqual => Some(Comparison::LessOrEqual),
        BinaryOperator::Greater => Some(Comparison::Greater),
        BinaryOperator::GreaterOrEqual => Some(Comparison::GreaterOrEqual),
        _ => None,
    }
}

impl Runtime<'_, '_> {
    pub(in crate::execution) fn declared_comparison(
        &mut self,
        comparison: Comparison,
        left_expression: &Expr,
        right_expression: &Expr,
        left: CellValue,
        right: CellValue,
    ) -> Result<CellValue, QueryError> {
        let declaration = combined(left.declaration, right.declaration);
        let policy =
            self.expression_pair_policy(left_expression, right_expression, &left, &right)?;
        let value = self
            .compare_expression_predicate_policy(comparison, &left, &right, policy)?
            .into_value();
        Ok(CellValue::scalar(value).with_declaration(declaration))
    }
}
