//! Owned selection state shared by buffered and progressive extrema.
use super::{CellValue, Expr, ExpressionValue, QueryError, Runtime, TextPolicy, Value};
use tablecraft_expressions::{EvalError, comparison::Comparison};

pub(in crate::execution) struct ExtremeState {
    selected: CellValue,
    policy: TextPolicy,
    complete: bool,
    maximum: bool,
    deferred: bool,
    candidates: Vec<CellValue>,
}

impl ExtremeState {
    pub(in crate::execution) fn new(maximum: bool, deferred: bool) -> Self {
        Self {
            selected: CellValue::scalar(Value::Null),
            policy: TextPolicy::Default,
            complete: false,
            maximum,
            deferred,
            candidates: Vec::new(),
        }
    }
}

impl Runtime<'_, '_> {
    /// The caller owns the per-row tick and operand evaluation. Keeping them
    /// outside this service preserves the buffered loop's exact error timing.
    pub(in crate::execution) fn push_extreme(
        &mut self,
        state: &mut ExtremeState,
        argument: &Expr,
        value: CellValue,
    ) -> Result<(), QueryError> {
        state.policy = state.policy.merge(value.policy);
        state.complete |= value.policy_complete;
        match &value.value {
            ExpressionValue::Value(Value::Null) => return Ok(()),
            ExpressionValue::Value(_) | ExpressionValue::StringBytes(_) => {}
            ExpressionValue::Error(_) => {
                // Standard-code admission applies even to an Error singleton.
                self.evaluator()
                    .compare_expression_values(Comparison::Equal, &value, &value)?;
            }
            ExpressionValue::Empty | ExpressionValue::Missing => {
                return Err(EvalError::NonScalarExpression.into());
            }
        }

        let string = matches!(
            value.value,
            ExpressionValue::Value(Value::Text(_)) | ExpressionValue::StringBytes(_)
        );
        if state.deferred && string {
            if !matches!(state.selected.value, ExpressionValue::Value(Value::Null)) {
                return Err(EvalError::TypeMismatch.into());
            }
            self.push(&mut state.candidates, value)?;
        } else {
            // Preserve strict String/non-String separation without activating
            // a provider; non-String populations retain one selected cell.
            if !state.candidates.is_empty() {
                return Err(EvalError::TypeMismatch.into());
            }
            if self.replace_extreme(
                argument,
                &state.selected,
                &value,
                state.policy,
                state.maximum,
            )? {
                state.selected = value;
            }
        }
        Ok(())
    }

    pub(in crate::execution) fn finish_extreme(
        &mut self,
        mut state: ExtremeState,
        argument: &Expr,
    ) -> Result<CellValue, QueryError> {
        // Only retained data is compared here. NULLs contributed to policy and
        // completion even though they never became value candidates.
        for value in state.candidates {
            self.tick()?;
            let value = value
                .with_policy(state.policy)
                .with_complete_policy(state.complete);
            if self.replace_extreme(
                argument,
                &state.selected,
                &value,
                state.policy,
                state.maximum,
            )? {
                state.selected = value;
            }
        }
        Ok(state
            .selected
            .with_policy(state.policy)
            .with_complete_policy(state.complete))
    }
}
