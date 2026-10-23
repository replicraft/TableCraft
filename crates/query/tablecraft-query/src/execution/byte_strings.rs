//! Lossless string-byte intermediates share the scalar evaluator's kernels.
use super::{Environment, Runtime};
use crate::{QueryError, plan::Expr};
use tablecraft_core::values::{ExpressionValue, Value};
use tablecraft_expressions::functions::text::{
    TextFunction,
    byte_string::{ByteOperand, ExpressionByteOperand},
};

impl Runtime<'_, '_> {
    fn expression_byte_operand(
        &mut self,
        expression: &Expr,
        env: Environment<'_>,
    ) -> Result<ExpressionByteOperand, QueryError> {
        match expression {
            Expr::TextCall(function, _) if function.produces_bytes() => {
                self.byte_operand(expression, env).map(Into::into)
            }
            Expr::CVar(value) => {
                self.tick()?;
                self.expression_byte_operand(value, env)
            }
            _ => self
                .expression_operand(expression, env)
                .map(|value| ExpressionByteOperand::Scalar(value.value)),
        }
    }

    pub(super) fn byte_operand(
        &mut self,
        expression: &Expr,
        env: Environment<'_>,
    ) -> Result<ByteOperand, QueryError> {
        match expression {
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
            (Some(self.eval(&arguments[0], env)?), &arguments[1..])
        } else {
            (None, arguments)
        };
        let haystack = self.byte_operand(&strings[0], env)?;
        let needle = self.byte_operand(&strings[1], env)?;
        let mode = strings
            .get(2)
            .map(|expression| self.eval(expression, env))
            .transpose()?;
        Ok(self
            .evaluator()
            .apply_byte_search(start.as_ref(), haystack, needle, mode.as_ref())?)
    }

    pub(super) fn byte_call(
        &mut self,
        function: TextFunction,
        arguments: &[Expr],
        env: Environment<'_>,
    ) -> Result<ByteOperand, QueryError> {
        if function == TextFunction::StrConv {
            let first = self.expression_byte_operand(&arguments[0], env)?;
            let mut rest: [ExpressionValue; 2] =
                std::array::from_fn(|_| ExpressionValue::Value(Value::Null));
            for (argument, value) in arguments[1..].iter().zip(&mut rest) {
                *value = self.expression_operand(argument, env)?.value;
            }
            return Ok(self.evaluator().apply_expression_strconv(
                first,
                &rest[0],
                (arguments.len() > 2).then_some(&rest[1]),
            )?);
        }
        let first = self.byte_operand(&arguments[0], env)?;
        let mut rest = [Value::Null, Value::Null];
        for (argument, value) in arguments[1..].iter().zip(&mut rest) {
            *value = self.eval(argument, env)?;
        }
        Ok(self
            .evaluator()
            .apply_byte_string(function, first, &rest[..arguments.len() - 1])?)
    }
}
