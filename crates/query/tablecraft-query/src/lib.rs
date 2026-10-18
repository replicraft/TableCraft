//! Bounded retrieval and snapshot-based UPDATE plans. No native file I/O.
//!
//! ```
//! use tablecraft_core::values::Value;
//! use tablecraft_schema::{Column, ScalarType};
//! use tablecraft_query::{prepare, ExecutionOptions, source::{MemorySource, MemoryTable}};
//! use tablecraft_sql::{parse_query, ParseLimits};
//! let source = MemorySource::new(vec![MemoryTable::new(
//!     "Items".into(), vec![Column::new("ID".into(), ScalarType::Int64, false)?],
//!     vec![vec![Value::Int64(2)], vec![Value::Int64(1)]],
//! )?])?;
//! let parsed = parse_query("SELECT ID FROM Items WHERE ID > 1", ParseLimits::default())?;
//! let plan = prepare(parsed.query(), &source)?;
//! let result = plan.execute(&source, &[], ExecutionOptions::default())?;
//! assert_eq!(result.rows(), &[vec![Value::Int64(2)]]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
#![deny(missing_docs)]
mod binding;
mod error;
mod execution;
mod plan;
mod profile;
pub mod source;
pub use binding::{PreparedQuery, prepare};
pub use error::QueryError;
pub use execution::{ExecutionOptions, QueryLimits, QueryResult};
pub use execution::{UpdateError, UpdatePlan, plan_update};
pub use source::Source;
