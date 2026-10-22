//! Legacy DATE/Text predicates reuse CDate without truncating extended carriers.
use super::{Evaluator, target};
use crate::{EvalError, functions::dates::parsing::DateConversion};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    pub(super) fn converted_comparison_operand(
        &mut self,
        value: &Value,
        other: &Value,
    ) -> Result<Option<Value>, EvalError> {
        if matches!((value, other), (Value::Text(_), Value::DateTime(_))) {
            return self
                .apply_date_conversion(DateConversion::CDate, value)
                .map(Some);
        }
        target(value, other)
            .map(|target| self.apply_conversion(target, value))
            .transpose()
    }
}

#[cfg(test)]
mod tests;
