//! Bounded retrieval and snapshot-based UPDATE plans. No native file I/O.
//!
//! ```
//! use tablecraft_core::values::Value;
//! use tablecraft_schema::{Column, ScalarType};
//! use tablecraft_query::{prepare, ExecutionOptions, OutputType, source::{MemorySource, MemoryTable}};
//! use tablecraft_sql::{parse_query, ParseLimits};
//! let source = MemorySource::new(vec![MemoryTable::new(
//!     "Items".into(), vec![Column::new("ID".into(), ScalarType::Int64, false)?],
//!     vec![vec![Value::Int64(2)], vec![Value::Int64(1)]],
//! )?])?;
//! let parsed = parse_query("SELECT ID FROM Items WHERE ID > 1", ParseLimits::default())?;
//! let plan = prepare(parsed.query(), &source)?;
//! assert!(matches!(plan.output_types(), Some([OutputType::Scalar { nullable: false, .. }])));
//! let result = plan.execute(&source, &[], ExecutionOptions::default())?;
//! assert_eq!(result.rows(), &[vec![Value::Int64(2)]]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
#![deny(missing_docs)]
mod binding;
mod collection_results;
pub use collection_results::{CollectionQueryResult, MultivalueValue, QueryValue};
mod discovery;
mod error;
pub use discovery::{execute_collections_with_schema_discovery, execute_with_schema_discovery};
mod execution;
mod external_sources;
mod heading_names;
mod named_identity;
mod output_types;
mod plan;
pub use output_types::OutputType;
mod profile;
pub mod source;
pub use binding::{BindingDepth, PreparationOptions, PreparedQuery, prepare, prepare_with_options};
pub use error::QueryError;
pub use execution::{
    DeleteError, DeletePlan, UpdateError, UpdatePlan, plan_batch_update, plan_delete, plan_update,
};
pub use execution::{
    ExecutionOptions, ExpressionQueryResult, QueryLimits, QueryResult, StreamError, StreamSummary,
};
pub use execution::{InsertError, local_insert_table, plan_insert};
pub use source::Source;
