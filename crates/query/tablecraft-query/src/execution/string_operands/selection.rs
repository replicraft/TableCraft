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

        let policy_complete = value.policy_complete
            || replacement
                .as_ref()
                .is_some_and(|value| value.policy_complete);
        Ok(StringCell {
            value: expression::nz_bytes(value.value, replacement.map(|value| value.value)),
            policy,
            policy_complete,
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
        let mut policy_complete = false;

        for argument in arguments {
            if selection.is_control() {
                let value = self
                    .expression_byte_operand(argument, env)?
                    .into_expression_value();
                self.evaluator()
                    .push_selection_control(&mut selection, value)?;
            } else {
                let value = self.string_cell(argument, env)?;
                policy = policy.merge(value.policy);
                policy_complete |= value.policy_complete;
                self.evaluator()
                    .push_selection_payload(&mut selection, value)?;
            }
        }

        let mut selected = match selection.finish_payload()? {
            Some(value) => value,
            None => StringCell::try_from(CellValue::scalar(Value::Null))?,
        };
        selected.policy = policy;
        selected.policy_complete = policy_complete;
        selected.declaration = Some(OperandDeclaration::Variant);
        Ok(selected)
    }
}
