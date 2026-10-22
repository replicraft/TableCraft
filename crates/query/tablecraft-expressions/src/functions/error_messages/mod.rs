//! Explicit localized message lookup, separate from operational/runtime Error state.
#[cfg(feature = "fluent-catalog")]
mod fluent;
#[cfg(feature = "fluent-catalog")]
pub use fluent::{EnglishErrorMessages, FluentErrorMessages};

use crate::EvalError;

/// Stable, bounded message data selected by the host for one execution.
/// Providers must be deterministic, perform no external effects and enforce
/// their own lookup bounds. Returned Text is copied under evaluator limits.
/// A valid undefined code uses the provider's explicit localized fallback.
/// Locale discovery, diagnostics and latest VBA runtime state are not implied.
pub trait ErrorMessages: Send + Sync {
    /// Borrow the exact localized message for a nonzero standard code.
    fn message(&self, code: u16) -> Result<&str, EvalError>;
}

#[cfg(all(test, feature = "fluent-catalog"))]
mod tests;
