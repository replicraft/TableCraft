//! Checked Long conversion with an explicit LongLong carrier exemption.
use super::{Literal, TextFunction, TextOutput};
use crate::{EvalError, functions::conversions::Conversion};
use std::fmt::Write;
use tablecraft_core::values::Value;

pub(super) fn write(
    function: TextFunction,
    value: &Value,
    output: &mut impl TextOutput,
) -> Result<(), EvalError> {
    let number = match value {
        Value::Int64(value) => *value,
        _ => {
            let Value::Int32(value) = Conversion::Long.apply(value)? else {
                return Err(EvalError::TypeMismatch);
            };
            i64::from(value)
        }
    };
    // These value-based intervals follow the published MS-VBAL Hex/Oct rules,
    // including their strict -32768 boundary. Native width qualification is open.
    let bits = if number < 0 && number > i64::from(i16::MIN) {
        u64::from(number as u16)
    } else if number <= i64::from(i16::MIN) {
        if number >= i64::from(i32::MIN) {
            u64::from(number as u32)
        } else {
            number as u64
        }
    } else {
        number as u64
    };
    let mut text = Literal::new();
    if function == TextFunction::Hex {
        write!(text, "{bits:X}")
    } else {
        write!(text, "{bits:o}")
    }
    .map_err(|_| EvalError::TextLimit)?;
    output.append(text.text()?)
}
