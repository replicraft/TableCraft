//! Eager variable-arity selection without buffering or copying argument payloads.
#[cfg(feature = "sql")]
mod context;
pub mod expression;
mod index;

use crate::EvalError;
use tablecraft_core::values::{ExpressionValue, Value};

/// Closed selector identities, independent of ASTs and application hosts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConditionalFunction {
    /// First true condition's paired value, otherwise NULL.
    Switch,
    /// Rounded one-based index's choice, otherwise NULL.
    Choose,
}
impl ConditionalFunction {
    /// Resolve only unqualified ASCII-insensitive function spellings.
    pub fn resolve(name: &str) -> Option<Self> {
        [("Switch", Self::Switch), ("Choose", Self::Choose)]
            .into_iter()
            .find_map(|(name_value, function)| {
                name.eq_ignore_ascii_case(name_value).then_some(function)
            })
    }
    /// Require at least one pair or one index/choice, with no missing slots.
    pub const fn accepts_arity(self, count: usize) -> bool {
        count >= 2 && (!matches!(self, Self::Switch) || count.is_multiple_of(2))
    }
    /// Begin an allocation-free selection. The caller owns work/argument limits
    /// and evaluates every expression in written order, even after selection.
    pub fn start(self, count: usize) -> Result<Selection, EvalError> {
        if !self.accepts_arity(count) {
            return Err(EvalError::UnsupportedExpression);
        }
        Ok(Selection {
            function: self,
            count,
            position: 0,
            chosen: None,
            take_next: false,
            value: None,
            error: None,
        })
    }
}
/// Incremental ownership transfer for evaluated arguments; intentionally no Debug.
/// Conversion errors are deferred until finish so later expression evaluation
/// errors remain observable, matching the scalar evaluator's eager call boundary.
pub struct Selection {
    function: ConditionalFunction,
    count: usize,
    position: usize,
    chosen: Option<usize>,
    take_next: bool,
    value: Option<ExpressionValue>,
    error: Option<EvalError>,
}
impl Selection {
    /// Consume the next evaluated value. Unselected payloads are dropped. Only an
    /// argument-count violation fails here; operand conversion errors wait for finish.
    pub fn push(&mut self, value: Value) -> Result<(), EvalError> {
        self.push_expression(ExpressionValue::Value(value))
    }

    /// Retain expression data in choice positions; conditions do not unwrap Error codes.
    pub fn push_expression(&mut self, value: ExpressionValue) -> Result<(), EvalError> {
        if self.position >= self.count {
            self.error = Some(EvalError::UnsupportedExpression);
            return Err(EvalError::UnsupportedExpression);
        }
        match self.function {
            ConditionalFunction::Choose if self.position == 0 => {
                match index::select(&value, self.count - 1) {
                    Ok(index) => self.chosen = index,
                    Err(error) => self.error = Some(error),
                }
            }
            ConditionalFunction::Choose => {
                if self.chosen == Some(self.position) {
                    self.value = Some(value);
                }
            }
            ConditionalFunction::Switch if self.position.is_multiple_of(2) => {
                match expression::condition(&value) {
                    Ok(condition) => self.take_next = self.value.is_none() && condition,
                    Err(error) => {
                        if self.error.is_none() {
                            self.error = Some(error);
                        }
                        self.take_next = false;
                    }
                }
            }
            ConditionalFunction::Switch => {
                if self.take_next {
                    self.value = Some(value);
                }
            }
        }
        self.position += 1;
        Ok(())
    }
    /// Return the chosen carrier unchanged after every declared argument arrived.
    pub fn finish(self) -> Result<Value, EvalError> {
        super::conversions::errors::into_scalar(self.finish_expression()?)
    }

    /// Finish without forcing selected expression-only data into stored scalars.
    pub fn finish_expression(self) -> Result<ExpressionValue, EvalError> {
        if self.position != self.count {
            return Err(EvalError::UnsupportedExpression);
        }
        if let Some(error) = self.error {
            return Err(error);
        }
        Ok(self.value.unwrap_or(ExpressionValue::Value(Value::Null)))
    }
}
#[cfg(test)]
mod tests;

#[cfg(test)]
mod coercion_tests;
