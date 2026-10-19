use thiserror::Error;

/// Evaluation failures without SQL, field names or value payloads.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum EvalError {
    /// A syntax form has no evaluator in this initial profile.
    #[error("expression form is not implemented")]
    UnsupportedExpression,
    /// The host has no value for the requested field/parameter.
    #[error("expression field has no binding")]
    UnboundField,
    /// Literal syntax cannot be materialized in this numeric profile.
    #[error("literal is outside the supported evaluation profile")]
    UnsupportedLiteral,
    /// The host requested cancellation.
    #[error("expression evaluation cancelled")]
    Cancelled,
    /// Maximum expression nesting exceeded.
    #[error("expression depth limit exceeded")]
    DepthLimit,
    /// Consumable operation budget exhausted.
    #[error(transparent)]
    Limit(#[from] tablecraft_core::limits::LimitError),
    /// Arithmetic operands are outside the operation's mathematical domain.
    #[error("invalid arithmetic domain")]
    Domain,
    /// The operation does not accept these scalar kinds.
    #[error("expression type mismatch")]
    TypeMismatch,
    /// A numeric result cannot fit its supported carrier.
    #[error("numeric overflow")]
    Overflow,
    /// A floating carrier contains NaN or infinity.
    #[error("non-finite number is not supported")]
    NonFinite,
    /// A divisor is zero.
    #[error("division by zero")]
    DivisionByZero,
    /// Implicit conversion would discard exact integer precision.
    #[error("numeric conversion would lose precision")]
    PrecisionLoss,
    /// Promotion/rounding for this pair is outside the implemented profile.
    #[error("numeric combination is not implemented")]
    UnsupportedNumericCombination,
    /// LIKE requires an explicit ANSI wildcard convention.
    #[error("LIKE requires a wildcard mode")]
    LikeModeRequired,
    /// An invalid wildcard list or descending range was supplied.
    #[error("invalid LIKE pattern")]
    InvalidPattern,
    /// String comparison needs an explicit collation implementation.
    #[error("text comparison requires a collation")]
    CollationRequired,
    /// A concatenated result exceeds the caller's byte ceiling.
    #[error("text output limit exceeded")]
    TextLimit,
    /// Memory could not be reserved for a text result.
    #[error("text allocation failed")]
    Allocation,
}
