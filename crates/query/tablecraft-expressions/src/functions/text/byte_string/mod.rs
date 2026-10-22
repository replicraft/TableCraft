//! Lossless expression byte strings, distinct from stored Text and Binary.
mod observations;
mod value;

pub(super) use observations::{first_byte, length};
use tablecraft_core::values::Value;
pub use value::ByteString;

/// An evaluated scalar or a lossless byte-string intermediate. No implicit
/// Binary-to-string conversion or unmetered cloning is provided.
#[derive(Debug)]
pub enum ByteOperand {
    /// An ordinary scalar, including NULL.
    Scalar(Value),
    /// Raw string bytes, including odd lengths and unpaired UTF-16 units.
    Bytes(ByteString),
}
