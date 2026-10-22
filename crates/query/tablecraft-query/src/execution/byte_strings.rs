//! Lossless string-byte intermediates share the scalar evaluator's kernels.
use super::{Environment, Runtime};
use crate::{QueryError, plan::Expr};
use tablecraft_core::values::Value;
use tablecraft_expressions::functions::text::{TextFunction, byte_string::ByteOperand};

impl Runtime<'_> {
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

    pub(super) fn byte_call(
        &mut self,
        function: TextFunction,
        arguments: &[Expr],
        env: Environment<'_>,
    ) -> Result<ByteOperand, QueryError> {
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
