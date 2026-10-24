//! Hyperlink selectors validate before source admission; addresses remain inert.
use super::{Evaluator, text_arguments};
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        text::{TextFunction, hyperlink},
    },
};
use tablecraft_core::{
    limits::Resource,
    values::{ExpressionValue, Value},
};

impl Evaluator<'_> {
    /// Admit eager states and explicit-policy numeric Text selectors without
    /// copying the hyperlink source or following its address.
    pub fn apply_expression_hyperlink_part(
        &mut self,
        arguments: &[ExpressionValue],
    ) -> Result<Value, EvalError> {
        if !TextFunction::HyperlinkPart.accepts_arity(arguments.len()) {
            return Err(EvalError::UnsupportedExpression);
        }
        self.text_expression_limits(arguments)?;
        self.check(0)?;

        let empty_number = Value::Int32(0);
        let empty_text = Value::Text(String::new());
        let selector = text_arguments::scalar(&arguments[1], &empty_number)?;
        let Value::Int32(selector) = self.apply_conversion(Conversion::Long, selector)? else {
            return Err(EvalError::TypeMismatch);
        };
        let result = hyperlink::apply_part(
            || text_arguments::scalar(&arguments[0], &empty_text),
            selector,
            || {
                if self.cancelled.is_some_and(|cancelled| cancelled()) {
                    return Err(EvalError::Cancelled);
                }
                self.budget.charge(Resource::Steps, 1)?;
                Ok(())
            },
        )?;
        self.materialize_text_result(result)
    }
}

#[cfg(test)]
mod tests;
