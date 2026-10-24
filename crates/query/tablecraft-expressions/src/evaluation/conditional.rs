//! Selector Text uses the same explicit, metered numeric policies as conversion calls.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        conditional::{Selection, expression},
        conversions::Conversion,
    },
};
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    /// Select already evaluated branches, preserving their carriers and Error data.
    /// Text conditions use the execution's explicit Boolean conversion policy.
    pub fn apply_iif(
        &mut self,
        condition: &ExpressionValue,
        yes: ExpressionValue,
        no: ExpressionValue,
    ) -> Result<ExpressionValue, EvalError> {
        if let ExpressionValue::Value(value @ Value::Text(_)) = condition {
            self.check(0)?;
            let condition = self.apply_conversion(Conversion::Boolean, value)?;
            expression::iif(&ExpressionValue::Value(condition), yes, no)
        } else {
            expression::iif(condition, yes, no)
        }
    }

    /// Consume one evaluated selector argument. Semantic conversion failures wait
    /// for finish; work, cancellation, allocation and other operational failures
    /// propagate immediately before any later argument can cause side effects.
    pub fn push_selection_argument(
        &mut self,
        selection: &mut Selection,
        value: ExpressionValue,
    ) -> Result<(), EvalError> {
        let Some(conversion) = selection.input_conversion() else {
            return selection.push_expression(value);
        };
        let ExpressionValue::Value(input @ Value::Text(_)) = &value else {
            return selection.push_expression(value);
        };
        if let Err(error) = self.check(0) {
            return selection.push_failure(error);
        }
        match self.apply_conversion(conversion, input) {
            Ok(value) => selection.push_expression(ExpressionValue::Value(value)),
            Err(error) => selection.push_failure(error),
        }
    }
}

#[cfg(test)]
mod tests;
