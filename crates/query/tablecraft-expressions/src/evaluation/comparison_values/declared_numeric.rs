//! A known Scalar numeric operand selects the width of String Let-coercion.
use super::Evaluator;
use crate::{
    EvalError, comparison::Comparison, functions::conversions::Conversion, logic::Truth,
    numeric::OperandDeclaration,
};
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    /// Compare a known non-Variant numeric operand with a String coerced to its
    /// selected carrier. The String's declaration need not be known: both Scalar
    /// and Variant String operands choose this numeric width. Other families
    /// return None for their existing comparison services. No declaration is
    /// inferred from a value, and NULL/Error/state handling remains separate.
    pub fn compare_scalar_numeric_string(
        &mut self,
        operator: Comparison,
        left: &ExpressionValue,
        right: &ExpressionValue,
        left_declaration: Option<OperandDeclaration>,
        right_declaration: Option<OperandDeclaration>,
    ) -> Result<Option<Truth>, EvalError> {
        let (numeric, string, numeric_left) = match (left, right) {
            (ExpressionValue::Value(value), string)
                if left_declaration == Some(OperandDeclaration::Scalar) && is_string(string) =>
            {
                (value, string, true)
            }
            (string, ExpressionValue::Value(value))
                if right_declaration == Some(OperandDeclaration::Scalar) && is_string(string) =>
            {
                (value, string, false)
            }
            _ => return Ok(None),
        };
        let conversion = match numeric {
            Value::Boolean(_) => Conversion::Boolean,
            Value::Byte(_) => Conversion::Byte,
            Value::Int16(_) => Conversion::Integer,
            Value::Int32(_) => Conversion::Long,
            Value::Int64(_) => Conversion::LongLong,
            Value::Single(_) => Conversion::Single,
            Value::Double(_) => Conversion::Double,
            // Currency, Decimal and DATE already have effective-type conversion
            // in the ordinary contextual service. Unsupported states stay there.
            _ => return Ok(None),
        };
        self.check(0)?;

        let published;
        let text = match string {
            ExpressionValue::Value(value) => value,
            ExpressionValue::StringBytes(bytes) => {
                published = self.publish_expression_string(bytes)?;
                &published
            }
            _ => return Ok(None),
        };
        let converted = self.apply_conversion(conversion, text)?;
        let result = if numeric_left {
            self.compare_values(operator, numeric, &converted)?
        } else {
            self.compare_values(operator, &converted, numeric)?
        };
        Ok(Some(result))
    }
}

fn is_string(value: &ExpressionValue) -> bool {
    matches!(
        value,
        ExpressionValue::Value(Value::Text(_)) | ExpressionValue::StringBytes(_)
    )
}

#[cfg(test)]
mod tests;
