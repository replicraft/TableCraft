//! String concatenation shares admission and provenance with other string consumers.
mod addition;

use super::{Environment, Runtime, cell::CellValue, string_operands::StringCell};
use crate::{
    QueryError,
    plan::{Expr, declarations::combined},
};
use tablecraft_expressions::functions::text::byte_string::ExpressionByteOperand;

impl Runtime<'_, '_> {
    pub(super) fn concatenation_operand(
        &mut self,
        left: &Expr,
        right: &Expr,
        env: Environment<'_>,
    ) -> Result<ExpressionByteOperand, QueryError> {
        self.concatenate_cells(left, right, env)
            .map(|cell| cell.value)
    }

    pub(super) fn concatenation_cell(
        &mut self,
        left: &Expr,
        right: &Expr,
        env: Environment<'_>,
    ) -> Result<CellValue, QueryError> {
        let cell = self.concatenate_cells(left, right, env)?;
        self.retain_string_cell(cell)
    }

    pub(super) fn concatenate_cells(
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
        let value = self
            .evaluator()
            .apply_expression_concatenation(left.value, right.value)?;
        Ok(StringCell {
            value: value.into(),
            policy,
            policy_complete: true,
            declaration,
        })
    }
}
