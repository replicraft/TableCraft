//! Bounded Access SQL syntax; no name resolution, evaluation or database access.
//!
//! ```
//! use tablecraft_sql::{parse_select, ParseLimits};
//! use tablecraft_sql::select::SelectPredicate;
//!
//! let parsed = parse_select(
//!     "SELECT DISTINCTROW [Label] FROM [Samples];",
//!     ParseLimits::default(),
//! )?;
//! assert_eq!(parsed.query().predicate, SelectPredicate::DistinctRow);
//! assert_eq!(parsed.query().to_string(), parsed.source());
//! # Ok::<(), tablecraft_sql::ParseError>(())
//! ```

#![deny(missing_docs)]

mod parser;
pub mod query;
pub mod select;

pub use parser::{
    ParseError, ParseErrorKind, ParseLimits, ParsedQuery, ParsedSelect, SourceLocation,
    parse_expression, parse_expression_strict, parse_field_rule, parse_query, parse_select,
};
