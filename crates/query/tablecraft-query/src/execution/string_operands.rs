//! Lossless scalar/string operands retain runtime policy and declaration provenance.
mod selection;

use super::{Environment, Runtime, cell::CellValue};
use crate::{
    QueryError,
    plan::{Expr, declarations::functions},
    source::TextPolicy,
};
use tablecraft_core::values::ExpressionValue;
use tablecraft_expressions::{
    functions::text::byte_string::ExpressionByteOperand, numeric::OperandDeclaration,
};
use tablecraft_sql::select::BinaryOperator;

pub(super) struct StringCell {
    pub(super) value: ExpressionByteOperand,
    pub(super) policy: TextPolicy,
    pub(super) declaration: Option<OperandDeclaration>,
}

impl TryFrom<CellValue> for StringCell {
    type Error = QueryError;

    fn try_from(cell: CellValue) -> Result<Self, Self::Error> {
        Ok(Self {
            value: ExpressionByteOperand::try_from(cell.value)?,
            policy: cell.policy,
            declaration: cell.declaration,
        })
    }
}

impl Runtime<'_, '_> {
    pub(super) fn iif_string_cell(
        &mut self,
        condition: &Expr,
        yes: &Expr,
        no: &Expr,
        env: Environment<'_>,
    ) -> Result<StringCell, QueryError> {
        self.tick()?;
        let condition = self.expression_operand(condition, env)?;
        let yes = self.string_cell(yes, env)?;
        let no = self.string_cell(no, env)?;
        let policy = yes.policy.merge(no.policy);
        let mut selected = self.evaluator().apply_iif(&condition, yes, no)?;
        selected.policy = policy;
        selected.declaration = Some(OperandDeclaration::Variant);

        Ok(selected)
    }

    pub(super) fn retain_string_cell(&mut self, cell: StringCell) -> Result<CellValue, QueryError> {
        let value = match cell.value {
            ExpressionByteOperand::Scalar(value) => value,
            ExpressionByteOperand::Bytes(bytes) => {
                ExpressionValue::StringBytes(bytes.into_expression_string())
            }
        };

        Ok(CellValue::from(value)
            .with_policy(cell.policy)
            .with_declaration(cell.declaration))
    }

    pub(super) fn string_cell(
        &mut self,
        expression: &Expr,
        env: Environment<'_>,
    ) -> Result<StringCell, QueryError> {
        let mut cell = match expression {
            Expr::IIf(condition, yes, no) => self.iif_string_cell(condition, yes, no, env),
            Expr::Nz(value, replacement) => self.nz_string_cell(value, replacement.as_deref(), env),
            Expr::ConditionalCall(function, arguments) => {
                self.conditional_string_cell(*function, arguments, env)
            }
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
                    policy: TextPolicy::Default,
                    declaration: functions::result(expression),
                })
            }
            _ => {
                return self
                    .retained_expression_operand(expression, env)
                    .and_then(StringCell::try_from);
            }
        }?;

        cell.policy = self.result_text_policy(expression, cell.policy, env)?;
        Ok(cell)
    }
}
