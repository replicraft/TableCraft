//! Lossless choice payloads share the existing metered control conversion.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        conditional::{ConditionalFunction, expression},
        text::byte_string::ExpressionByteOperand,
    },
};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_sql::select::FunctionArgument;

impl Evaluator<'_> {
    pub(in crate::evaluation) fn nz_byte_operand(
        &mut self,
        arguments: &[FunctionArgument],
        depth: usize,
    ) -> Result<ExpressionByteOperand, EvalError> {
        let [FunctionArgument::Expression(value), rest @ ..] = arguments else {
            return Err(EvalError::UnsupportedExpression);
        };
        if rest.len() > 1
            || rest
                .iter()
                .any(|argument| !matches!(argument, FunctionArgument::Expression(_)))
        {
            return Err(EvalError::UnsupportedExpression);
        }
        self.check(depth)?;

        let value = self.expression_byte_operand(value, depth + 1)?;
        let replacement = match rest.first() {
            Some(FunctionArgument::Expression(value)) => {
                Some(self.expression_byte_operand(value, depth + 1)?)
            }
            _ => None,
        };

        Ok(expression::nz_bytes(value, replacement))
    }

    pub(in crate::evaluation) fn selection_byte_operand(
        &mut self,
        function: ConditionalFunction,
        arguments: &[FunctionArgument],
        depth: usize,
    ) -> Result<ExpressionByteOperand, EvalError> {
        if !function.accepts_arity(arguments.len())
            || arguments
                .iter()
                .any(|argument| !matches!(argument, FunctionArgument::Expression(_)))
        {
            return Err(EvalError::UnsupportedExpression);
        }
        self.check(depth)?;
        let mut selection = function.start_payload(arguments.len())?;

        for argument in arguments {
            let FunctionArgument::Expression(value) = argument else {
                unreachable!("argument kinds validated before evaluation");
            };
            if selection.is_control() {
                let value = self.expression_operand(value, depth + 1)?;
                self.push_selection_control(&mut selection, value)?;
            } else {
                let value = self.expression_byte_operand(value, depth + 1)?;
                self.push_selection_payload(&mut selection, value)?;
            }
        }

        Ok(selection
            .finish_payload()?
            .unwrap_or(ExpressionByteOperand::Scalar(ExpressionValue::Value(
                Value::Null,
            ))))
    }
}
