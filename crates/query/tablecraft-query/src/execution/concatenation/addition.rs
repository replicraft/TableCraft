//! Numeric additions retain the existing declared dispatch and value provenance.
use super::{CellValue, Environment, Expr, QueryError, Runtime, StringCell, combined};
use tablecraft_expressions::{
    evaluation::PreparedAddition, functions::text::byte_string::ExpressionByteOperand,
};
use tablecraft_sql::select::BinaryOperator;

impl Runtime<'_, '_> {
    pub(in crate::execution) fn addition_operand(
        &mut self,
        left: &Expr,
        right: &Expr,
        env: Environment<'_>,
    ) -> Result<ExpressionByteOperand, QueryError> {
        self.add_cells(left, right, env).map(|cell| cell.value)
    }

    pub(in crate::execution) fn addition_cell(
        &mut self,
        left: &Expr,
        right: &Expr,
        env: Environment<'_>,
    ) -> Result<CellValue, QueryError> {
        let cell = self.add_cells(left, right, env)?;
        self.retain_string_cell(cell)
    }

    pub(in crate::execution) fn add_cells(
        &mut self,
        left: &Expr,
        right: &Expr,
        env: Environment<'_>,
    ) -> Result<StringCell, QueryError> {
        self.tick()?;
        let left = self.string_cell(left, env)?;
        let right = self.string_cell(right, env)?;
        let policy = left
            .contributing_policy()
            .merge(right.contributing_policy());
        let declaration = combined(left.declaration, right.declaration);
        let prepared = if self.options.declared_integral {
            self.evaluator()
                .prepare_declared_expression_addition(
                    left.value,
                    right.value,
                    left.declaration,
                    right.declaration,
                )?
                .ok_or(QueryError::ArithmeticDeclaration)?
        } else {
            self.evaluator()
                .prepare_expression_addition(left.value, right.value)?
        };

        match prepared {
            PreparedAddition::StringResult(value) => Ok(StringCell {
                value: value.into(),
                policy,
                policy_complete: true,
                declaration,
            }),
            PreparedAddition::ScalarOperands {
                left: left_value,
                right: right_value,
            } => {
                let left = CellValue::scalar(left_value)
                    .with_policy(left.policy)
                    .with_declaration(left.declaration);
                let right = CellValue::scalar(right_value)
                    .with_policy(right.policy)
                    .with_declaration(right.declaration);
                self.declared_binary(BinaryOperator::Add, left, right)
                    .and_then(StringCell::try_from)
            }
        }
    }
}
