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
        let search = search::prepare(function, arguments)?;
        let start = search.start().ok_or(EvalError::UnsupportedExpression)?;
        let Value::Int32(start) = self.apply_conversion(Conversion::Long, start)? else {
            return Err(EvalError::TypeMismatch);
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
