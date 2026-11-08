//! Shared evaluation inputs and immutable row provenance for mutations.
mod options;
pub use options::MutationOptions;
pub(in crate::execution) mod source;
