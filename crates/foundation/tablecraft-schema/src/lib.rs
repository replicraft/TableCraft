//! Scalar relation contracts, independent of query syntax and storage engines.
//!
//! ```
//! use tablecraft_schema::{Column, NameComparison, SchemaPolicy, TypeDescriptor, validate_row};
//! use tablecraft_core::values::Value;
//! let columns = vec![Column::with_type("Label".into(), TypeDescriptor::text(2)?, false)?];
//! let profile = SchemaPolicy { max_name_bytes: 64, max_columns: 16, names: NameComparison::Exact };
//! profile.validate_columns(&columns)?;
//! validate_row(&columns, &[Value::Text("é🦀".into())])?;
//! assert!(validate_row(&columns, &[Value::Text("long".into())]).is_err());
//! # Ok::<(), tablecraft_schema::SchemaError>(())
//! ```
#![deny(missing_docs)]
mod column;
mod constraints;
mod error;
mod name;
mod policy;
mod relationship;
mod scalar;
mod type_descriptor;
pub use column::{Column, validate_columns, validate_row};
pub use constraints::{FieldConstraintViolation, FieldConstraints};
pub use error::SchemaError;
pub use name::{NameComparison, SchemaName};
pub use policy::SchemaPolicy;
pub use relationship::{ReferentialAction, Relationship, RelationshipColumn, RelationshipRules};
pub use scalar::ScalarType;
pub use type_descriptor::TypeDescriptor;
