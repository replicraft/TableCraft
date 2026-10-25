//! Declaration-dependent String comparison; retained values and strict keys stay separate.
use super::Evaluator;
use crate::{
    EvalError,
    comparison::Comparison,
    functions::text::TextFunction,
    logic::Truth,
    numeric::{ArithmeticDeclarations, OperandDeclaration},
};
use tablecraft_core::values::{ExpressionValue, Value};

/// Prepared coercion for one known Scalar String / Variant comparison.
/// Existing operands remain borrowed; at most one newly formatted peer is owned.
pub enum PreparedScalarStringVariantComparison {
    /// Original operands already have the required representation or state.
    Original,
    /// The left Variant was formatted once under explicit CStr policies.
    ConvertedLeft(Value),
    /// The right Variant was formatted once under explicit CStr policies.
    ConvertedRight(Value),
}

impl Evaluator<'_> {
    /// Compare a known Scalar String with a known Variant through String coercion.
    /// Other declaration/carrier pairs return None. NULL and expression states
    /// retain the ordinary comparison boundary; Error is not implicitly formatted.
    /// Existing retained Strings keep their raw representation and provider rules.
    pub fn compare_scalar_string_variant(
        &mut self,
        operator: Comparison,
        left: &ExpressionValue,
        right: &ExpressionValue,
        declarations: ArithmeticDeclarations,
    ) -> Result<Option<Truth>, EvalError> {
        let Some(prepared) =
            self.prepare_scalar_string_variant_comparison(left, right, declarations)?
        else {
            return Ok(None);
        };
        let left_owned;
        let right_owned;
        let (left, right) = match prepared {
            PreparedScalarStringVariantComparison::Original => (left, right),
            PreparedScalarStringVariantComparison::ConvertedLeft(value) => {
                left_owned = ExpressionValue::Value(value);
                (&left_owned, right)
            }
            PreparedScalarStringVariantComparison::ConvertedRight(value) => {
                right_owned = ExpressionValue::Value(value);
                (left, &right_owned)
            }
        };
        self.compare_expression_values(operator, left, right)
            .map(Some)
    }

    /// Prepare CStr coercion before a host activates the String comparison provider.
    /// Conversion failures remain eager and metered; no source operand is cloned.
    pub fn prepare_scalar_string_variant_comparison(
        &mut self,
        left: &ExpressionValue,
        right: &ExpressionValue,
        declarations: ArithmeticDeclarations,
    ) -> Result<Option<PreparedScalarStringVariantComparison>, EvalError> {
        let string_left = match (declarations.left, declarations.right) {
            (OperandDeclaration::Scalar, OperandDeclaration::Variant) if string(left) => true,
            (OperandDeclaration::Variant, OperandDeclaration::Scalar) if string(right) => false,
            _ => return Ok(None),
        };
        self.check(0)?;
        let peer = if string_left { right } else { left };
        let converted = match peer {
            ExpressionValue::Empty => Some(self.text("")?),
            ExpressionValue::Value(value) if stringifiable(value) => {
                Some(self.apply_text_function(TextFunction::CStr, std::slice::from_ref(value))?)
            }
            _ => None,
        };
        let prepared = match converted {
            Some(value) if string_left => {
                PreparedScalarStringVariantComparison::ConvertedRight(value)
            }
            Some(value) => PreparedScalarStringVariantComparison::ConvertedLeft(value),
            None => PreparedScalarStringVariantComparison::Original,
        };
        Ok(Some(prepared))
    }
}

fn string(value: &ExpressionValue) -> bool {
    matches!(
        value,
        ExpressionValue::StringBytes(_) | ExpressionValue::Value(Value::Text(_))
    )
}

fn stringifiable(value: &Value) -> bool {
    matches!(
        value,
        Value::Boolean(_)
            | Value::Byte(_)
            | Value::Int16(_)
            | Value::Int32(_)
            | Value::Int64(_)
            | Value::Single(_)
            | Value::Double(_)
            | Value::Currency(_)
            | Value::Decimal(_)
            | Value::DateTime(_)
    )
}

#[cfg(test)]
mod tests;
