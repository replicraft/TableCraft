//! Budgeted type-name output and explicit array/state inspection.
use super::{Evaluator, text_output::Output};
use crate::{
    EvalError,
    functions::{
        inspection::types::{InspectionInput, InspectionResult, ValueInspectionFunction},
        text::output::TextOutput,
    },
};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    /// Inspect an evaluated scalar, array or host-issued expression-only state.
    /// Text results use the same output limits as other string functions.
    pub fn apply_value_inspection(
        &mut self,
        function: ValueInspectionFunction,
        input: InspectionInput<'_>,
    ) -> Result<Value, EvalError> {
        self.check(0)?;
        match function.apply(input)? {
            InspectionResult::Scalar(value) => Ok(value),
            InspectionResult::TypeName(name) => {
                let mut output = Output {
                    budget: self.budget,
                    cancelled: self.cancelled,
                    max_bytes: self.limits.max_text_bytes,
                    text: String::new(),
                };
                output.append(name)?;
                Ok(Value::Text(output.text))
            }
        }
    }
}
