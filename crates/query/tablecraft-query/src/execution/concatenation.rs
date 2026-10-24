//! Lossless string operands retain runtime provenance until final publication.
mod addition;

use super::{Environment, Runtime, cell::CellValue};
use crate::{
    QueryError,
    plan::{
        Expr,
        declarations::{combined, functions},
    },
    source::TextPolicy,
};
use tablecraft_core::values::ExpressionValue;
use tablecraft_expressions::{
    functions::text::byte_string::{ByteOperand, ExpressionByteOperand},
    numeric::OperandDeclaration,
};
use tablecraft_sql::select::BinaryOperator;

struct StringCell {
    value: ExpressionByteOperand,
    policy: TextPolicy,
    declaration: Option<OperandDeclaration>,
}

impl From<CellValue> for StringCell {
    fn from(cell: CellValue) -> Self {
        Self {
            value: ExpressionByteOperand::Scalar(cell.value),
            policy: cell.policy,
            declaration: cell.declaration,
        }
    }
}

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
        self.publish_string_cell(cell)
    }

    fn publish_string_cell(&mut self, cell: StringCell) -> Result<CellValue, QueryError> {
        let value = match cell.value {
            ExpressionByteOperand::Scalar(value) => value,
            ExpressionByteOperand::Bytes(bytes) => ExpressionValue::Value(
                self.evaluator()
                    .publish_byte_operand(ByteOperand::Bytes(bytes))?,
            ),
        };
        Ok(CellValue::from(value)
            .with_policy(cell.policy)
            .with_declaration(cell.declaration))
    }

    fn concatenate_cells(
        &mut self,
        left: &Expr,
        right: &Expr,
        env: Environment<'_>,
    ) -> Result<StringCell, QueryError> {
        self.tick()?;
        let left = self.string_cell(left, env)?;
        let right = self.string_cell(right, env)?;
        let policy = left.policy.merge(right.policy);
        let declaration = combined(left.declaration, right.declaration);
        let value = self
            .evaluator()
            .apply_expression_concatenation(left.value, right.value)?;
        Ok(StringCell {
            value: value.into(),
            policy,
            declaration,
        })
    }

    fn string_cell(
        &mut self,
        expression: &Expr,
        env: Environment<'_>,
    ) -> Result<StringCell, QueryError> {
        match expression {
            Expr::Binary(BinaryOperator::Add, left, right) => self.add_cells(left, right, env),
            Expr::Binary(BinaryOperator::Concatenate, left, right) => {
                self.concatenate_cells(left, right, env)
            }
            Expr::CVar(value) => {
                self.tick()?;
                let mut value = self.string_cell(value, env)?;
                value.declaration = Some(OperandDeclaration::Variant);
                Ok(value)
            }
            Expr::TextCall(function, arguments) if function.produces_bytes() => {
                self.tick()?;
                let value = self.byte_call(*function, arguments, env)?;
                Ok(StringCell {
                    value: value.into(),
                    policy: self.text_policy(expression)?,
                    declaration: functions::result(expression),
                })
            }
            _ => self.expression_operand(expression, env).map(Into::into),
        }
    }
}
