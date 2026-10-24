//! Lossless string-byte intermediates share the scalar evaluator's kernels.
use super::{Environment, Runtime};
use crate::{QueryError, plan::Expr};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::functions::text::{
    TextFunction,
    byte_string::{ByteOperand, ExpressionByteOperand},
};
use tablecraft_sql::select::BinaryOperator;

impl Runtime<'_, '_> {
    pub(super) fn expression_byte_operand(
        &mut self,
        expression: &Expr,
        env: Environment<'_>,
    ) -> Result<ExpressionByteOperand, QueryError> {
        match expression {
            Expr::Nz(..) | Expr::ConditionalCall(..) => {
                self.string_cell(expression, env).map(|value| value.value)
            }
            Expr::IIf(condition, yes, no) => self
                .iif_string_cell(condition, yes, no, env)
                .map(|value| value.value),
            Expr::Binary(BinaryOperator::Add, left, right) => {
                self.addition_operand(left, right, env)
            }
            Expr::Binary(BinaryOperator::Concatenate, left, right) => {
                self.concatenation_operand(left, right, env)
            }
            Expr::TextCall(function, _) if function.produces_bytes() => {
                self.byte_operand(expression, env).map(Into::into)
            }
            Expr::CVar(value) => {
                self.tick()?;
                self.expression_byte_operand(value, env)
            }
            _ => self
                .retained_expression_operand(expression, env)
                .and_then(|value| ExpressionByteOperand::try_from(value.value).map_err(Into::into)),
        }
    }

    pub(super) fn byte_operand(
        &mut self,
        expression: &Expr,
        env: Environment<'_>,
    ) -> Result<ByteOperand, QueryError> {
        match expression {
            Expr::Binary(
                operator @ (BinaryOperator::Add | BinaryOperator::Concatenate),
                left,
                right,
            ) => {
                let value = if *operator == BinaryOperator::Add {
                    self.addition_operand(left, right, env)?
                } else {
                    self.concatenation_operand(left, right, env)?
                };
                Ok(match value {
                    ExpressionByteOperand::Scalar(ExpressionValue::Value(value)) => {
                        ByteOperand::Scalar(value)
                    }
                    ExpressionByteOperand::Bytes(bytes) => ByteOperand::Bytes(bytes),
                    _ => return Err(QueryError::Unsupported),
                })
            }
            Expr::TextCall(function, arguments) if function.produces_bytes() => {
                self.tick()?;
                self.byte_call(*function, arguments, env)
            }
            Expr::CVar(value) => {
                self.tick()?;
                self.byte_operand(value, env)
            }
            _ => self.eval(expression, env).map(ByteOperand::Scalar),
        }
    }

    pub(super) fn byte_search_call(
        &mut self,
        arguments: &[Expr],
        env: Environment<'_>,
    ) -> Result<Value, QueryError> {
        let (start, strings) = if arguments.len() > 2 {
            (
                Some(self.expression_operand(&arguments[0], env)?.value),
                &arguments[1..],
            )
        } else {
            (None, arguments)
        };
        let haystack = self.expression_byte_operand(&strings[0], env)?;
        let needle = self.expression_byte_operand(&strings[1], env)?;
        let mode = strings
            .get(2)
            .map(|expression| {
                self.expression_operand(expression, env)
                    .map(|value| value.value)
            })
            .transpose()?;
        Ok(self.evaluator().apply_expression_byte_search(
            start.as_ref(),
            haystack,
            needle,
            mode.as_ref(),
        )?)
    }

    pub(super) fn byte_call(
        &mut self,
        function: TextFunction,
        arguments: &[Expr],
        env: Environment<'_>,
    ) -> Result<ByteOperand, QueryError> {
        if function == TextFunction::String {
            let count = self
                .expression_byte_operand(&arguments[0], env)?
                .into_expression_value();
            let character = self
                .expression_byte_operand(&arguments[1], env)?
                .into_expression_value();
            return Ok(self.evaluator().apply_retained_string(&count, &character)?);
        }
        if function == TextFunction::Replace {
            let mut values: [ExpressionValue; TextFunction::MAX_ARGUMENTS] =
                std::array::from_fn(|_| ExpressionValue::Value(Value::Null));
            for (index, (argument, value)) in arguments.iter().zip(&mut values).enumerate() {
                *value = if index < 3 {
                    self.expression_byte_operand(argument, env)?
                        .into_expression_value()
                } else {
                    self.expression_operand(argument, env)?.value
                };
            }
            return Ok(self
                .evaluator()
                .apply_retained_replacement(&values[..arguments.len()])?);
        }
        let (first, rest) = self.byte_call_inputs(arguments, env)?;
        Ok(self.evaluator().apply_expression_byte_string(
            function,
            first,
            &rest[..arguments.len() - 1],
        )?)
    }

    pub(super) fn slice_call(
        &mut self,
        function: TextFunction,
        arguments: &[Expr],
        env: Environment<'_>,
    ) -> Result<Value, QueryError> {
        let (first, rest) = self.byte_call_inputs(arguments, env)?;
        Ok(self.evaluator().apply_expression_slice_source(
            function,
            first,
            &rest[..arguments.len() - 1],
        )?)
    }

    fn byte_call_inputs(
        &mut self,
        arguments: &[Expr],
        env: Environment<'_>,
    ) -> Result<(ExpressionByteOperand, [ExpressionValue; 2]), QueryError> {
        let first = self.expression_byte_operand(&arguments[0], env)?;
        let mut rest: [ExpressionValue; 2] =
            std::array::from_fn(|_| ExpressionValue::Value(Value::Null));
        for (argument, value) in arguments[1..].iter().zip(&mut rest) {
            *value = self.expression_operand(argument, env)?.value;
        }
        Ok((first, rest))
    }
}
