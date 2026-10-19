//! Explicit scalar built-ins over already evaluated materialized values.
use crate::{EvalError, numeric};
use tablecraft_core::values::Value;

/// Closed built-in identities; lookup never resolves external or user functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Builtin {
    /// IsNull(value), distinct from empty text or zero.
    IsNull,
    /// IIf(condition, truepart, falsepart), with eager argument evaluation.
    IIf,
}
impl Builtin {
    /// ASCII-insensitive unqualified built-in lookup, without allocation.
    pub fn resolve(name: &str) -> Option<Self> {
        if name.eq_ignore_ascii_case("IsNull") {
            Some(Self::IsNull)
        } else if name.eq_ignore_ascii_case("IIf") {
            Some(Self::IIf)
        } else {
            None
        }
    }
}
/// Whether the materialized operand is SQL NULL. Empty text/binary and zero are not.
/// Expression-only Empty/Missing/Error states are not accepted as stored values.
pub const fn is_null(value: &Value) -> bool {
    matches!(value, Value::Null)
}

/// Select and move one already evaluated branch without coercion or allocation.
/// Callers must evaluate condition, truepart and falsepart in written order before
/// calling, including the unselected branch. This function is not short-circuiting.
/// Boolean/exact numeric/finite floating conditions use zero=false, nonzero=true;
/// NULL selects falsepart. Text/date/binary/GUID truth conversion is unsupported.
pub fn iif(condition: &Value, truepart: Value, falsepart: Value) -> Result<Value, EvalError> {
    let selected = if is_null(condition) {
        false
    } else if let Some((coefficient, _)) = numeric::fixed(condition) {
        coefficient != 0
    } else {
        numeric::float(condition)? != 0.0
    };
    Ok(if selected { truepart } else { falsepart })
}

#[cfg(test)]
mod tests;
