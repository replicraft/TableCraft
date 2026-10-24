//! Lossless expression byte strings, distinct from stored Text and Binary.
mod comparison;
mod input;
mod observations;
mod search;
mod value;

pub use comparison::compare_binary_utf16;
pub use input::ExpressionByteOperand;
pub(super) use observations::{first_byte, length};
pub use search::find_binary;
#[cfg(feature = "sql")]
pub(crate) use search::mode as search_mode;
use tablecraft_core::values::Value;
pub use value::ByteString;
#[cfg(feature = "sql")]
pub(crate) use value::write_utf8;

/// An evaluated scalar or a lossless byte-string intermediate. No implicit
/// Binary-to-string conversion or unmetered cloning is provided.
#[derive(Debug)]
pub enum ByteOperand {
    /// An ordinary scalar, including NULL.
    Scalar(Value),
    /// Raw string bytes, including odd lengths and unpaired UTF-16 units.
    Bytes(ByteString),
}
