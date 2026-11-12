//! Bounded Access SELECT design, preserving unsupported SQL by explicit refusal.
mod build;
mod criteria;
mod field_cell;
mod import;
mod model;
mod parameters;
mod totals;

pub use build::{build, build_ast, draft_text_bytes, expression_text, validate_metadata};
pub use field_cell::{edit_field_operand, field_cell_complete, field_cell_text, parse_field_cell};
pub use import::{import, import_unbound};
pub use model::{
    BuiltQueryDesign, DesignColumn, DesignExpression, DesignField, DesignFieldMetadata, DesignJoin,
    DesignJoinKind, DesignSort, DesignSource, DesignTableMetadata, DesignTotal, QueryDesignDraft,
};

pub use parameters::{parse_parameters, render_parameters};
pub use totals::validate_totals_off;

use thiserror::Error;

pub const MAX_DESIGN_SOURCES: usize = 8;
pub const MAX_DESIGN_JOINS: usize = 7;
pub const MAX_DESIGN_COLUMNS: usize = 32;
pub const MAX_DESIGN_CRITERIA_ROWS: usize = 8;
pub const MAX_DESIGN_TEXT_BYTES: usize = 64 * 1024;
pub const MAX_DESIGN_METADATA_FIELDS: usize = 1024;
pub const MAX_DESIGN_METADATA_BYTES: usize = 1024 * 1024;

#[derive(Debug, Error)]
pub enum QueryDesignError {
    #[error("query design exceeds the bounded editor profile")]
    Limit,
    #[error("SQL cannot be represented by this query design without losing clauses")]
    Unsupported,
    #[error("query design contains an invalid or ambiguous source or field")]
    InvalidReference,
    #[error("query design must show at least one output field")]
    NoProjection,
    #[error("the join graph cannot be represented by the current design profile")]
    InvalidJoins,
    #[error(transparent)]
    Parse(#[from] tablecraft_sql::ParseError),
}

#[cfg(test)]
mod tests;
