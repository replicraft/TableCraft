//! Date/numeric effective comparison types reuse existing checked conversions.
use super::{Evaluator, numeric};
use crate::{
    EvalError,
    comparison::{self, Comparison},
    functions::{conversions::Conversion, dates::parsing::DateConversion},
    logic::Truth,
};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    pub(in crate::evaluation::comparison_values) fn compare_effective_date_values(
        &mut self,
        operator: Comparison,
        left: &Value,
        right: &Value,
    ) -> Result<Option<Truth>, EvalError> {
        let other = match (left, right) {
            (Value::DateTime(_), Value::DateTime(_)) => return Ok(None),
            (Value::DateTime(_), other) | (other, Value::DateTime(_)) => other,
            _ => return Ok(None),
        };
        if !numeric(other) {
            // Date/Text and Date/Date already have their own conversion/order.
            // Extended Date and other stored carriers are not silently truncated.
            return Ok(None);
        }
        let (left, right) = if matches!(other, Value::Decimal(_)) {
            (
                self.apply_conversion(Conversion::Decimal, left)?,
                self.apply_conversion(Conversion::Decimal, right)?,
            )
        } else {
            (
                self.apply_date_conversion(DateConversion::CDate, left)?,
                self.apply_date_conversion(DateConversion::CDate, right)?,
            )
        };
        comparison::compare(operator, &left, &right, None).map(Some)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod materialized_tests;
