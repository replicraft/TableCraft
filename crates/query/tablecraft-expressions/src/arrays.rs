//! Bounded expression-only arrays; stored scalar values remain unchanged.
mod bounds;
mod value;

pub use bounds::{ArrayView, BoundFunction};
pub use value::ArrayValue;

#[cfg(feature = "sql")]
pub mod sql_arguments;

#[cfg(test)]
mod tests;
