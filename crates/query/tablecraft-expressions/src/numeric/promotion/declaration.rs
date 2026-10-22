//! Explicit declared-overflow handling for nullable integral operands.
use super::{integral_target, promoted_arithmetic};
use crate::{
    EvalError,
    functions::conversions::Conversion,
    numeric::{Arithmetic, integer},
};
use tablecraft_core::values::Value;

/// Caller-established declaration category, separate from a value's carrier.
/// This does not infer Access SQL declarations from runtime values or schemas.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OperandDeclaration {
    /// The runtime carrier is the caller's declared scalar type.
    Scalar,
    /// A declared Variant currently holding the supplied integral carrier.
    #[default]
    Variant,
}

/// Declaration categories for one evaluated binary operation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArithmeticDeclarations {
    /// Declaration category established before evaluating the left operand.
    pub left: OperandDeclaration,
    /// Declaration category established before evaluating the right operand.
    pub right: OperandDeclaration,
}
impl ArithmeticDeclarations {
    /// Result declaration for this profile; any Variant participant retains Variant.
    pub const fn result(self) -> OperandDeclaration {
        match (self.left, self.right) {
            (OperandDeclaration::Scalar, OperandDeclaration::Scalar) => OperandDeclaration::Scalar,
            _ => OperandDeclaration::Variant,
        }
    }
}

/// Apply the current integral arithmetic profile with explicit overflow provenance.
///
/// Inputs must be nullable Boolean/Byte/Int16/Int32/Int64 carriers. Two declared
/// scalar inputs prohibit widening of integral results. A Variant participant
/// retains existing materialized widening; LongLong overflow still fails. `/` and
/// `^` retain the current Double profile. NULL is returned only after input-family
/// validation. This bounded, allocation-free kernel does not evaluate operands,
/// infer static types, establish ACE SQL declarations, or qualify native behavior.
pub fn declared_integral_arithmetic(
    operator: Arithmetic,
    left: &Value,
    right: &Value,
    declarations: ArithmeticDeclarations,
) -> Result<Value, EvalError> {
    for value in [left, right] {
        if !matches!(value, Value::Null) && integer(value).is_none() {
            return Err(EvalError::TypeMismatch);
        }
    }
    let result = promoted_arithmetic(operator, left, right)?;
    if declarations.result() == OperandDeclaration::Variant
        || matches!(result, Value::Null)
        || matches!(operator, Arithmetic::Divide | Arithmetic::Power)
    {
        return Ok(result);
    }
    let retains_carrier = matches!(
        (integral_target(left, right), &result),
        (Conversion::Byte, Value::Byte(_))
            | (Conversion::Integer, Value::Int16(_))
            | (Conversion::Long, Value::Int32(_))
            | (Conversion::LongLong, Value::Int64(_))
    );
    if !retains_carrier {
        return Err(EvalError::Overflow);
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
