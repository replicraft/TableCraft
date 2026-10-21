//! Explicit scalar built-ins over already evaluated materialized values.
pub mod conditional;
pub mod conversions;
pub mod dates;
pub mod financial;
pub mod inspection;
pub mod math;
pub mod text;

use crate::{EvalError, arrays::BoundFunction, numeric};
use tablecraft_core::values::Value;

/// Closed built-in identities; lookup never resolves external or user functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Builtin {
    /// IsNull(value), distinct from empty text or zero.
    IsNull,
    /// Preserve an already materialized argument value and its carrier.
    CVar,
    /// Parse an Access GUID spelling into sixteen native GUID-structure bytes.
    GuidFromString,
    /// IIf(condition, truepart, falsepart), with eager argument evaluation.
    IIf,
    /// Query-context NULL replacement, with an optional fallback.
    Nz,
    /// Eager variable-arity Switch/Choose selection.
    Conditional(conditional::ConditionalFunction),
    /// Gregorian interval arithmetic over explicit date/time carriers.
    DateAdd,
    /// Signed Gregorian boundary counts with explicit optional week settings.
    DateDiff,
    /// Gregorian component selection with explicit optional week settings.
    DatePart,
    /// Standalone Gregorian weekday with optional first-day policy.
    Weekday,
    /// Host-issued current date/time, without implicit engine clock lookup.
    Clock(dates::clock::ClockFunction),
    /// Numeric Gregorian date/time constructors.
    DateConstructor(dates::DateConstructor),
    /// Closed text operations over Text/NULL values.
    Text(text::TextFunction),
    /// Closed numeric magnitude, sign, integral and rounding operations.
    Math(math::MathFunction),
    /// Fixed-rate annuity calculations over explicit numeric values.
    Financial(financial::FinancialFunction),
    /// Periodic cashflows supplied separately from stored scalar operands.
    CashFlow(financial::cashflows::CashFlowFunction),
    /// Gregorian extraction from explicit date/time carriers.
    Date(dates::DateFunction),
    /// Explicit date conversion/extraction with deterministic text policies.
    DateConvert(dates::parsing::DateConversion),
    /// Materialized scalar inspection with explicit numeric Text policy.
    Inspect(inspection::InspectionFunction),
    /// Inspect explicit scalar/array types and expression-only states.
    ValueInspect(inspection::types::ValueInspectionFunction),
    /// Inspect a single dimension of an explicit array.
    ArrayBound(BoundFunction),
    /// Explicit checked numeric/Boolean conversion functions.
    Convert(conversions::Conversion),
}
impl Builtin {
    /// ASCII-insensitive unqualified built-in lookup, without allocation.
    pub fn resolve(name: &str) -> Option<Self> {
        if name.eq_ignore_ascii_case("IsNull") {
            Some(Self::IsNull)
        } else if name.eq_ignore_ascii_case("CVar") {
            Some(Self::CVar)
        } else if name.eq_ignore_ascii_case("GUIDFromString") {
            Some(Self::GuidFromString)
        } else if name.eq_ignore_ascii_case("IIf") {
            Some(Self::IIf)
        } else if name.eq_ignore_ascii_case("Nz") {
            Some(Self::Nz)
        } else if name.eq_ignore_ascii_case("DateAdd") {
            Some(Self::DateAdd)
        } else if name.eq_ignore_ascii_case("DateDiff") {
            Some(Self::DateDiff)
        } else if name.eq_ignore_ascii_case("Weekday") {
            Some(Self::Weekday)
        } else if name.eq_ignore_ascii_case("DatePart") {
            Some(Self::DatePart)
        } else {
            text::TextFunction::resolve(name)
                .map(Self::Text)
                .or_else(|| conditional::ConditionalFunction::resolve(name).map(Self::Conditional))
                .or_else(|| math::MathFunction::resolve(name).map(Self::Math))
                .or_else(|| financial::FinancialFunction::resolve(name).map(Self::Financial))
                .or_else(|| {
                    financial::cashflows::CashFlowFunction::resolve(name).map(Self::CashFlow)
                })
                .or_else(|| dates::DateFunction::resolve(name).map(Self::Date))
                .or_else(|| dates::clock::ClockFunction::resolve(name).map(Self::Clock))
                .or_else(|| dates::DateConstructor::resolve(name).map(Self::DateConstructor))
                .or_else(|| conversions::Conversion::resolve(name).map(Self::Convert))
                .or_else(|| inspection::InspectionFunction::resolve(name).map(Self::Inspect))
                .or_else(|| {
                    inspection::types::ValueInspectionFunction::resolve(name)
                        .map(Self::ValueInspect)
                })
                .or_else(|| BoundFunction::resolve(name).map(Self::ArrayBound))
                .or_else(|| dates::parsing::DateConversion::resolve(name).map(Self::DateConvert))
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
    Ok(if self::condition(condition)? {
        truepart
    } else {
        falsepart
    })
}
fn condition(value: &Value) -> Result<bool, EvalError> {
    Ok(if is_null(value) {
        false
    } else if let Some((coefficient, _)) = numeric::fixed(value) {
        coefficient != 0
    } else {
        numeric::float(value)? != 0.0
    })
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
