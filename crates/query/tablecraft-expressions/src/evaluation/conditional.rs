//! Selector Text uses the same explicit, metered numeric policies as conversion calls.
mod strings;

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
    /// Move one already evaluated branch, preserving the caller's payload type.
    /// This permits lossless Strings as well as ordinary expression data.
    /// Text conditions use the execution's explicit Boolean conversion policy.
    pub fn apply_iif<T>(
        &mut self,
        condition: &ExpressionValue,
        yes: T,
        no: T,
    ) -> Result<T, EvalError> {
        self.check(0)?;

        if let ExpressionValue::Value(value @ Value::Text(_)) = condition {
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
        if selection.is_control() {
            self.push_selection_control(selection, value)
        } else {
            self.push_selection_payload(selection, value)
        }
    }

    /// Meter one choice without interpreting or converting its payload.
    pub fn push_selection_payload<T>(
        &mut self,
        selection: &mut Selection<T>,
        value: T,
    ) -> Result<(), EvalError> {
        if let Err(error) = self.check(0) {
            return selection.push_failure(error);
        }
        selection.push_payload(value)
    }

    /// Convert a control slot under the existing eager/deferred error contract.
    pub fn push_selection_control<T>(
        &mut self,
        selection: &mut Selection<T>,
        value: ExpressionValue,
    ) -> Result<(), EvalError> {
        if let Err(error) = self.check(0) {
            return selection.push_failure(error);
        }

        let Some(conversion) = selection.input_conversion() else {
            return selection.push_control(value);
        };
        let ExpressionValue::Value(input @ Value::Text(_)) = &value else {
            return selection.push_control(value);
        };
        match self.apply_conversion(conversion, input) {
            Ok(value) => selection.push_control(ExpressionValue::Value(value)),
            Err(error) => selection.push_failure(error),
        }
    }
}

#[cfg(test)]
mod tests;
