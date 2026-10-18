//! Scalar relation contracts, independent of query syntax and storage engines.
#![deny(missing_docs)]
mod column;
mod scalar;
pub use column::{Column, SchemaError, validate_columns, validate_row};
pub use scalar::ScalarType;
