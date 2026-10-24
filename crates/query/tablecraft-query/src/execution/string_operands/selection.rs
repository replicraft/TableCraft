//! Eager lossless choices retain branch metadata without buffering every payload.
use super::{CellValue, Environment, Runtime, StringCell};
use crate::{QueryError, plan::Expr, source::TextPolicy};
use tablecraft_core::values::Value;
use tablecraft_expressions::{
    functions::conditional::{ConditionalFunction, expression},
    numeric::OperandDeclaration,
};

impl Runtime<'_, '_> {
    pub(in crate::execution) fn nz_string_cell(
        &mut self,
        value: &Expr,
        replacement: Option<&Expr>,
        env: Environment<'_>,
    ) -> Result<StringCell, QueryError> {
        self.tick()?;
        let value = self.string_cell(value, env)?;
        let replacement = replacement
            .map(|replacement| self.string_cell(replacement, env))
            .transpose()?;
        let policy = value.policy.merge(
            replacement
                .as_ref()
                .map_or(TextPolicy::Default, |value| value.policy),
        );

        Ok(StringCell {
            value: expression::nz_bytes(value.value, replacement.map(|value| value.value)),
            policy,
            declaration: Some(OperandDeclaration::Variant),
        })
    }

    pub(in crate::execution) fn conditional_string_cell(
        &mut self,
        function: ConditionalFunction,
        arguments: &[Expr],
        env: Environment<'_>,
    ) -> Result<StringCell, QueryError> {
        self.tick()?;
        let mut selection = function.start_payload(arguments.len())?;
        let mut policy = TextPolicy::Default;

        for argument in arguments {
            if selection.is_control() {
                let value = self.expression_operand(argument, env)?;
                self.evaluator()
                    .push_selection_control(&mut selection, value.value)?;
            } else {
                let value = self.string_cell(argument, env)?;
                policy = policy.merge(value.policy);
                self.evaluator()
                    .push_selection_payload(&mut selection, value)?;
            }
        }

        let mut selected = selection
            .finish_payload()?
            .unwrap_or_else(|| CellValue::scalar(Value::Null).into());
        selected.policy = policy;
        selected.declaration = Some(OperandDeclaration::Variant);
        Ok(selected)
    }
}
