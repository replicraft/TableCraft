//! Explicit standard Error-to-String spelling; opaque host codes need a mapping.
use super::super::{TextResult, numeric_format, output::TextOutput};
use crate::{EvalError, functions::conversions::text::NumberFormat};
use tablecraft_core::values::Value;

const ERROR_PREFIX: &str = "Error ";

/// Write the English standard Error spelling without interpreting host HRESULTs.
/// The existing numeric renderer supplies invariant ungrouped code digits.
/// Hosts must discard partial output on failure and enforce cumulative limits.
pub fn write_standard_error(code: u32, output: &mut impl TextOutput) -> Result<(), EvalError> {
    let code = u16::try_from(code).map_err(|_| EvalError::UnsupportedExpression)?;
    let value = Value::Int32(i32::from(code));
    let TextResult::NumericFormat(recipe) =
        numeric_format::prepare_cstr(&value, NumberFormat::Invariant)?
    else {
        return Err(EvalError::TypeMismatch);
    };
    output.append(ERROR_PREFIX)?;
    recipe.write(output)
}
