//! Format-independent synchronous reads with shared work limits and cancellation.
//!
//! Callers charge requested bytes before I/O; these helpers charge read/seek work.
//! Individual blocking callbacks remain responsible for their own timeouts.

mod error;
pub use error::IoError;
mod read;
pub use read::{read_exact, read_exact_at};
