use thiserror::Error;

/// Evaluation failures without SQL, field names or value payloads.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum EvalError {
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
