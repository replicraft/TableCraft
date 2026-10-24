//! Expression data flows only through explicit constructors, selectors and consumers.
use super::{Evaluator, text_output::Output};
use crate::{
    EvalError,
    functions::{
        Builtin,
        conversions::{Conversion, errors},
        text::byte_string::write_utf8,
    },
};
use tablecraft_core::values::{ExpressionString, ExpressionValue, Value};
use tablecraft_sql::select::{Expression, FunctionArgument as A};

impl Evaluator<'_> {
    /// Evaluate expression data without forcing language-only states into stored scalars.
    /// The caller must use explicit consumers or a deliberate output policy.
    pub fn evaluate_expression_value(
        &mut self,
        expression: &Expression,
    ) -> Result<ExpressionValue, EvalError> {
        let value = self.expression_operand(expression, 0)?;
        self.publish_expression_value(value)
    }

    /// Explicitly publish a lossless String as Unicode Text under this evaluator's
    /// limits. Other expression states retain their existing identity.
    pub fn publish_expression_value(
        &mut self,
        value: ExpressionValue,
    ) -> Result<ExpressionValue, EvalError> {
        match value {
            ExpressionValue::StringBytes(value) => self
                .publish_expression_string(&value)
                .map(ExpressionValue::Value),
            value => Ok(value),
        }
    }

    /// Publish borrowed lossless String bytes under the existing UTF-8 output
    /// limits and budget, without copying the raw backing allocation.
    pub fn publish_expression_string(
        &mut self,
        value: &ExpressionString,
    ) -> Result<Value, EvalError> {
        i32::try_from(value.len()).map_err(|_| EvalError::Overflow)?;
        let mut output = Output {
            budget: self.budget,
            cancelled: self.cancelled,
            max_bytes: self.limits.max_text_bytes,
            text: String::new(),
        };
        write_utf8(value.as_bytes(), &mut output)?;
        Ok(Value::Text(output.text))
    }

    /// Construct Error data under this evaluator's conversion policy and budget.
    pub fn apply_cverr(&mut self, value: ExpressionValue) -> Result<ExpressionValue, EvalError> {
        self.check(0)?;
        let value = match value {
            ExpressionValue::Value(value @ Value::Text(_)) => {
                ExpressionValue::Value(self.apply_conversion(Conversion::Long, &value)?)
            }
            value => value,
        };
        errors::cverr(value)
    }

    /// Explicit numeric calls consume Empty as zero and reviewed calls consume
    /// standard Error codes. CDec requires separate numeric code extraction.
    /// Arithmetic and stored-value publication never convert states implicitly.
    pub fn apply_expression_conversion(
        &mut self,
        conversion: Conversion,
        value: &ExpressionValue,
    ) -> Result<Value, EvalError> {
        match value {
            ExpressionValue::Value(value) => self.apply_conversion(conversion, value),
            _ => {
                self.check(0)?;
                errors::convert(conversion, value)
            }
        }
    }

    pub(super) fn expression_operand(
        &mut self,
        expression: &Expression,
        depth: usize,
    ) -> Result<ExpressionValue, EvalError> {
        if let Expression::Call {
            qualifier,
            name,
            arguments,
        } = expression
            && qualifier.is_empty()
        {
            match (Builtin::resolve(name.as_str()), arguments.as_slice()) {
                (Some(Builtin::Eval), [A::Expression(value)]) => {
                    self.check(depth)?;
                    let value = self.node(value, depth + 1)?;
                    return self.dynamic_expression(&value, depth);
                }
                (Some(Builtin::CvErr), [A::Expression(value)]) => {
                    self.check(depth)?;
                    let value = self.expression_operand(value, depth + 1)?;
                    return self.apply_cverr(value);
                }
                (Some(Builtin::CVar), [A::Expression(value)]) => {
                    self.check(depth)?;
                    return self.expression_operand(value, depth + 1);
                }
                (
                    Some(Builtin::IIf),
                    [
                        A::Expression(condition),
                        A::Expression(yes),
                        A::Expression(no),
                    ],
                ) => {
                    self.check(depth)?;
                    let value = self.iif_byte_operand(condition, yes, no, depth)?;
                    return self.publish_expression_byte_value(value);
                }
                (Some(Builtin::Nz), [A::Expression(_), rest @ ..])
                    if rest.len() <= 1
                        && rest
                            .iter()
                            .all(|argument| matches!(argument, A::Expression(_))) =>
                {
                    let value = self.nz_byte_operand(arguments, depth)?;
                    return self.publish_expression_byte_value(value);
                }
                (Some(Builtin::Conditional(function)), arguments)
                    if function.accepts_arity(arguments.len())
                        && arguments
                            .iter()
                            .all(|argument| matches!(argument, A::Expression(_))) =>
                {
                    let value = self.selection_byte_operand(function, arguments, depth)?;
                    return self.publish_expression_byte_value(value);
                }
                _ => {}
            }
        }
        self.node(expression, depth).map(ExpressionValue::Value)
    }
}
