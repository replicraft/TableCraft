//! Scalar Access expression rules over core values, independent of databases.
//!
//! Supported numeric combinations and explicit limitations are recorded in
//! `docs/EXPRESSIONS.md`. This is not a VBA Variant interpreter or Access oracle.
#![deny(missing_docs)]

pub mod comparison;
mod error;
#[cfg(feature = "sql")]
pub mod evaluation;
pub mod functions;
pub mod logic;
pub mod numeric;
pub mod predicates;
pub mod text;
pub use error::EvalError;

#[cfg(test)]
mod test_support;

#[cfg(feature = "sql")]
pub mod datetime;

#[cfg(feature = "sql")]
pub mod guid;
