//! Double-selected LongLong predicates; strict key comparison stays separate.
use super::Evaluator;
use crate::{
    EvalError,
    comparison::{self, Comparison},
    functions::conversions::Conversion,
    logic::Truth,
};
use tablecraft_core::values::Value;

impl Evaluator<'_> {
    pub(super) fn compare_floating_integer(
        &mut self,
        operator: Comparison,
        left: &Value,
        right: &Value,
    ) -> Result<Option<Truth>, EvalError> {
        let pair = matches!(
            (left, right),
            (
                Value::Int64(_),
                Value::Single(_) | Value::Double(_) | Value::Text(_)
            ) | (
                Value::Single(_) | Value::Double(_) | Value::Text(_),
                Value::Int64(_)
            )
        );
        if !pair {
            return Ok(None);
        }
        // Convert both sides: retaining Single here would make the strict
        // Single/Double comparer narrow the rounded LongLong back to Single.
        let left = self.apply_conversion(Conversion::Double, left)?;
        let right = self.apply_conversion(Conversion::Double, right)?;
        comparison::compare(operator, &left, &right, None).map(Some)
    }
}

#[cfg(test)]
mod tests;
