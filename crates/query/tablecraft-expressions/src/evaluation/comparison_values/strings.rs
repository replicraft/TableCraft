//! Retained String comparison under an explicitly selected raw-unit provider.
use super::Evaluator;
use crate::{
    EvalError,
    comparison::{Comparison, StringComparisonProfile, StringInput, compare_utf16_strings},
    logic::Truth,
};
use tablecraft_core::values::{ExpressionValue, Value};

impl Evaluator<'_> {
    pub(super) fn compare_retained_strings(
        &mut self,
        operator: Comparison,
        left: &ExpressionValue,
        right: &ExpressionValue,
    ) -> Result<Option<Truth>, EvalError> {
        let raw = matches!(left, ExpressionValue::StringBytes(_))
            || matches!(right, ExpressionValue::StringBytes(_));
        if !raw {
            return Ok(None);
        }
        let Some(collation) = self.collation else {
            return Ok(None);
        };
        if collation.string_comparison_profile() != StringComparisonProfile::Utf16 {
            return Ok(None);
        }
        let (Some(left), Some(right)) = (input(left), input(right)) else {
            return Ok(None);
        };

        self.check(0)?;
        for operand in [left, right] {
            if matches!(operand, StringInput::Text(text) if text.len() > self.limits.max_text_bytes)
            {
                return Err(EvalError::TextLimit);
            }
        }
        compare_utf16_strings(
            operator,
            left,
            right,
            collation,
            self.budget,
            self.cancelled,
        )
        .map(Some)
    }
}

fn input(value: &ExpressionValue) -> Option<StringInput<'_>> {
    match value {
        ExpressionValue::Value(Value::Text(text)) => Some(StringInput::Text(text)),
        ExpressionValue::StringBytes(bytes) => Some(StringInput::Utf16Le(bytes.as_bytes())),
        _ => None,
    }
}
