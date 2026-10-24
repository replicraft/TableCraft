//! Eager variable-arity selection without buffering or copying argument payloads.
#[cfg(feature = "sql")]
mod context;
pub mod expression;
mod index;
mod selection;

pub use selection::Selection;

use crate::EvalError;

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
        self.start_payload(count)
    }

    /// Begin selection of opaque payloads. Control slots still consume checked
    /// expression data; callers evaluate every slot in written order.
    pub fn start_payload<T>(self, count: usize) -> Result<Selection<T>, EvalError> {
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
#[cfg(test)]
mod tests;

#[cfg(test)]
mod coercion_tests;
