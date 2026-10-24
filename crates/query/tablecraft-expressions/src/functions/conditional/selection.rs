//! Incremental control admission and owned choice selection.
use super::{ConditionalFunction, expression, index};
use crate::{EvalError, functions::conversions::errors};
use tablecraft_core::values::{ExpressionValue, Value};

/// Incremental ownership transfer for evaluated arguments; intentionally no Debug.
/// Conversion errors are deferred until finish so later expression evaluation
/// errors remain observable, matching the scalar evaluator's eager call boundary.
pub struct Selection<T = ExpressionValue> {
    pub(super) function: ConditionalFunction,
    pub(super) count: usize,
    pub(super) position: usize,
    pub(super) chosen: Option<usize>,
    pub(super) take_next: bool,
    pub(super) value: Option<T>,
    pub(super) error: Option<EvalError>,
}
impl Selection {
    /// Consume the next evaluated value. Unselected payloads are dropped. Only an
    /// argument-count violation fails here; operand conversion errors wait for finish.
    pub fn push(&mut self, value: Value) -> Result<(), EvalError> {
        self.push_expression(ExpressionValue::Value(value))
    }

    /// Retain expression data in choice positions; conditions do not unwrap Error codes.
    pub fn push_expression(&mut self, value: ExpressionValue) -> Result<(), EvalError> {
        if self.is_control() {
            self.push_control(value)
        } else {
            self.push_payload(value)
        }
    }

    /// Return the chosen carrier unchanged after every declared argument arrived.
    pub fn finish(self) -> Result<Value, EvalError> {
        errors::into_scalar(self.finish_expression()?)
    }

    /// Finish without forcing selected expression-only data into stored scalars.
    pub fn finish_expression(self) -> Result<ExpressionValue, EvalError> {
        Ok(self
            .finish_payload()?
            .unwrap_or(ExpressionValue::Value(Value::Null)))
    }
}

impl<T> Selection<T> {
    /// Whether the next declared slot is a condition or index, rather than a choice.
    pub fn is_control(&self) -> bool {
        self.position < self.count
            && match self.function {
                ConditionalFunction::Choose => self.position == 0,
                ConditionalFunction::Switch => self.position.is_multiple_of(2),
            }
    }

    fn validate_slot(&mut self, control: bool) -> Result<(), EvalError> {
        if self.position >= self.count || self.is_control() != control {
            self.error = Some(EvalError::UnsupportedExpression);
            return Err(EvalError::UnsupportedExpression);
        }
        Ok(())
    }

    /// Consume a control expression; semantic errors remain latched until finish.
    /// This does not inspect or convert any choice payload.
    pub fn push_control(&mut self, value: ExpressionValue) -> Result<(), EvalError> {
        self.validate_slot(true)?;

        match self.function {
            ConditionalFunction::Choose => match index::select(&value, self.count - 1) {
                Ok(index) => self.chosen = index,
                Err(error) => self.error = Some(error),
            },
            ConditionalFunction::Switch => match expression::condition(&value) {
                Ok(condition) => self.take_next = self.value.is_none() && condition,
                Err(error) => {
                    self.error.get_or_insert(error);
                    self.take_next = false;
                }
            },
        }
        self.position += 1;
        Ok(())
    }

    /// Move a choice payload, retaining only the selected choice. A selected NULL
    /// payload remains selected; absence is represented separately by Option.
    pub fn push_payload(&mut self, value: T) -> Result<(), EvalError> {
        self.validate_slot(false)?;

        let take = match self.function {
            ConditionalFunction::Choose => self.chosen == Some(self.position),
            ConditionalFunction::Switch => self.take_next,
        };
        if take {
            self.value = Some(value);
        }
        self.position += 1;
        Ok(())
    }

    /// Finish after all declared slots arrived. None means no choice matched;
    /// the owning consumer supplies its own NULL representation.
    pub fn finish_payload(self) -> Result<Option<T>, EvalError> {
        if self.position != self.count {
            return Err(EvalError::UnsupportedExpression);
        }
        if let Some(error) = self.error {
            return Err(error);
        }
        Ok(self.value)
    }
}
