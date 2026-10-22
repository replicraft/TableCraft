//! Date component sources use the same explicit, bounded conversion as CDate.
use super::Evaluator;
use crate::{
    EvalError,
    functions::dates::{DateFunction, parsing::DateConversion},
};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    /// Extract a Gregorian component from an evaluated date, numeric or Text source.
    /// Existing dates retain their carrier/precision; NULL propagates without a
    /// conversion policy. Other inputs share CDate's explicit syntax and limits.
    pub fn apply_date_function(
        &mut self,
        function: DateFunction,
        value: &Value,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        match value {
            Value::Null | Value::DateTime(_) | Value::DateTimeExtended(_) => function.apply(value),
            value => {
                let converted = self.apply_date_conversion(DateConversion::CDate, value)?;
                function.apply(&converted)
            }
        }
    }
}

#[cfg(test)]
mod tests;
