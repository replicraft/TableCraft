//! Explicit scalar built-ins over already evaluated materialized values.
pub mod text;

use crate::{EvalError, numeric};
use tablecraft_core::values::Value;

/// Closed built-in identities; lookup never resolves external or user functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Builtin {
    /// IsNull(value), distinct from empty text or zero.
    IsNull,
    /// IIf(condition, truepart, falsepart), with eager argument evaluation.
    IIf,
    /// Query-context NULL replacement, with an optional fallback.
    Nz,
    /// Closed text operations over Text/NULL values.
    Text(text::TextFunction),
}
impl Builtin {
    /// ASCII-insensitive unqualified built-in lookup, without allocation.
    pub fn resolve(name: &str) -> Option<Self> {
        if name.eq_ignore_ascii_case("IsNull") {
            Some(Self::IsNull)
        } else if name.eq_ignore_ascii_case("IIf") {
            Some(Self::IIf)
        } else if name.eq_ignore_ascii_case("Nz") {
            Some(Self::Nz)
        } else {
            text::TextFunction::resolve(name).map(Self::Text)
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

/// Query-context Nz over evaluated values, preserving the selected carrier.
/// An omitted fallback produces empty text for NULL, while an explicit NULL
/// fallback remains NULL. This does not implement VBA's contextual Empty value.
/// Callers evaluate both supplied operands before selecting a result.
pub fn nz_query(value: Value, replacement: Option<Value>) -> Value {
    if is_null(&value) {
        replacement.unwrap_or_else(|| Value::Text(String::new()))
    } else {
        value
    }
}

#[cfg(test)]
mod tests;
