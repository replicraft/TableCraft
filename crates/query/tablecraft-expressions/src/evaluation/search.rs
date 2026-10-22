//! Numeric Text search starts retain prepared comparison policy and borrowed strings.
use super::Evaluator;
use crate::{
    EvalError,
    functions::{
        conversions::Conversion,
        text::{TextFunction, TextResult, search},
    },
};
use tablecraft_core::{limits::Resource, values::Value};

impl Evaluator<'_> {
    pub(super) fn contextual_search(
        &mut self,
        function: TextFunction,
        arguments: &[Value],
    ) -> Result<TextResult<'static>, EvalError> {
        self.check(0)?;
        // The comparison mode is validated before start conversion, retaining
        // existing mode/NULL/error precedence without duplicating that parser.
        let mode = self.text_long_override(arguments.get(3))?;
        let search = search::prepare_mode(function, arguments, mode.as_ref())?;
        let start = match search.start() {
            Some(value) => match self.apply_conversion(Conversion::Long, value)? {
                Value::Int32(value) => value,
                _ => return Err(EvalError::TypeMismatch),
            },
            None => return Err(EvalError::UnsupportedExpression),
        };
        search.find(start, self.collation, &mut || {
            if self.cancelled.is_some_and(|cancelled| cancelled()) {
                return Err(EvalError::Cancelled);
            }
            self.budget.charge(Resource::Steps, 1)?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests;
