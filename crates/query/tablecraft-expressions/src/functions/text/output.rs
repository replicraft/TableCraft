//! Shared output contract for bounded text-producing kernels.
use crate::EvalError;

/// Host-owned work and output accounting. Discard partial output on any error;
/// append must enforce byte ceilings and charge before fallible allocation.
pub trait TextOutput {
    /// Charge work and check cancellation before scanning or provider invocation.
    fn step(&mut self) -> Result<(), EvalError>;
    /// Append a borrowed fragment under the host's cumulative output budget.
    fn append(&mut self, fragment: &str) -> Result<(), EvalError>;
}

/// Charged normalization storage. It must start empty, return exactly the appended
/// text and retain it for evaluation. Discard contents on error; append must charge
/// payload bytes before fallible allocation, as with other text output kernels.
pub trait NumericBuffer: TextOutput {
    /// Borrow the normalized prefix accumulated so far.
    fn text(&self) -> &str;
}

/// Host-owned raw string-byte construction. Charge work and bytes before each
/// fallible append; enforce a per-value ceiling and discard partial output on error.
pub trait ByteOutput {
    /// Charge work and check cancellation before scanning input.
    fn step(&mut self) -> Result<(), EvalError>;
    /// Append exact string bytes without Unicode decoding, padding or replacement.
    fn append(&mut self, fragment: &[u8]) -> Result<(), EvalError>;
}
