//! Evaluator-only conversion slots; selector payloads retain their original carriers.
use super::{ConditionalFunction, Selection};
use crate::{EvalError, functions::conversions::Conversion};

impl<T> Selection<T> {
    pub(crate) fn input_conversion(&self) -> Option<Conversion> {
        if self.position >= self.count {
            return None;
        }
        match self.function {
            ConditionalFunction::Choose if self.position == 0 => Some(Conversion::Single),
            ConditionalFunction::Switch if self.position.is_multiple_of(2) => {
                Some(Conversion::Boolean)
            }
            _ => None,
        }
    }

    /// Defer ordinary conversion failures, but abort and latch operational failures.
    /// An ignored error can never produce a successful finish.
    pub(crate) fn push_failure(&mut self, error: EvalError) -> Result<(), EvalError> {
        if self.position >= self.count {
            self.error = Some(EvalError::UnsupportedExpression);
            return Err(EvalError::UnsupportedExpression);
        }
        self.position += 1;
        self.take_next = false;
        match error {
            EvalError::TypeMismatch
            | EvalError::Domain
            | EvalError::Overflow
            | EvalError::NonFinite
            | EvalError::PrecisionLoss
            | EvalError::UnsupportedNumericCombination
            | EvalError::InvalidNullConversion
            | EvalError::NumberFormatRequired => {
                self.error.get_or_insert(error);
                Ok(())
            }
            _ => {
                self.error = Some(error);
                Err(error)
            }
        }
    }
}
