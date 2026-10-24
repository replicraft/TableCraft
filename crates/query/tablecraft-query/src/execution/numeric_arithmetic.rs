//! Raw numeric String operands preserve the original declared-dispatch boundary.
use super::{Environment, Runtime, cell::CellValue};
use crate::{
    QueryError,
    plan::{Expr, declarations::combined},
};
use tablecraft_expressions::evaluation::PreparedNumericArithmetic;
use tablecraft_sql::select::BinaryOperator;

impl Runtime<'_, '_> {
    pub(super) fn numeric_arithmetic_cell(
        &mut self,
        operator: BinaryOperator,
        left: &Expr,
        right: &Expr,
        env: Environment<'_>,
    ) -> Result<CellValue, QueryError> {
        self.tick()?;
        let left = self.string_cell(left, env)?;
        let right = self.string_cell(right, env)?;
        let declaration = combined(left.declaration, right.declaration);

        match self.evaluator().prepare_expression_numeric_arithmetic(
            operator,
            left.value,
            right.value,
        )? {
            PreparedNumericArithmetic::Result(value) => {
                Ok(CellValue::scalar(value).with_declaration(declaration))
            }
            PreparedNumericArithmetic::Operands {
                left: left_value,
                right: right_value,
            } => {
                let left = CellValue::scalar(left_value)
                    .with_policy(left.policy)
                    .with_declaration(left.declaration);
                let right = CellValue::scalar(right_value)
                    .with_policy(right.policy)
                    .with_declaration(right.declaration);
                self.declared_binary(operator, left, right)
            }
        }
    }
}
