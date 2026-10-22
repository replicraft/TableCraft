//! Expression-only Error data has explicit consumers and cannot become stored cells.
use super::{Environment, Runtime};
use crate::{QueryError, plan::Expr};
use tablecraft_core::values::ExpressionValue;
use tablecraft_expressions::functions::conditional::expression as selection;

impl Runtime<'_> {
    pub(super) fn expression_operand(
        &mut self,
        expression: &Expr,
        env: Environment<'_>,
    ) -> Result<ExpressionValue, QueryError> {
        match expression {
            Expr::CvErr(value) => {
                self.tick()?;
                let value = self.expression_operand(value, env)?;
                Ok(self.evaluator().apply_cverr(value)?)
            }
            Expr::CVar(value) => {
                self.tick()?;
                self.expression_operand(value, env)
            }
            Expr::IIf(condition, yes, no) => {
                self.tick()?;
                let condition = self.expression_operand(condition, env)?;
                let yes = self.expression_operand(yes, env)?;
                let no = self.expression_operand(no, env)?;
                Ok(selection::iif(&condition, yes, no)?)
            }
            Expr::Nz(value, replacement) => {
                self.tick()?;
                let value = self.expression_operand(value, env)?;
                let replacement = replacement
                    .as_ref()
                    .map(|replacement| self.expression_operand(replacement, env))
                    .transpose()?;
                Ok(selection::nz(value, replacement))
            }
            Expr::ConditionalCall(function, arguments) => {
                self.tick()?;
                let mut selection = function.start(arguments.len())?;
                for argument in arguments {
                    selection.push_expression(self.expression_operand(argument, env)?)?;
                }
                Ok(selection.finish_expression()?)
            }
            _ => self.eval(expression, env).map(ExpressionValue::Value),
        }
    }
}
