//! TableCraft-owned syntax for the supported single-table SELECT subset.

mod expression;
mod format;
mod name;

pub use expression::{BinaryOperator, Expression};
pub use name::{FieldReference, Name};

/// A SELECT syntax model, without binding, coercion or execution semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Select {
    /// Row selection syntax. TOP is not a promise of LIMIT-like execution.
    pub predicate: SelectPredicate,
    /// Ordered output columns.
    pub projection: Vec<Projection>,
    /// The single source table.
    pub from: Table,
    /// Optional WHERE expression.
    pub selection: Option<Expression>,
    /// Ordered sort keys.
    pub order_by: Vec<SortKey>,
}

/// Access selection predicates, kept distinct for later semantic analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectPredicate {
    /// ALL, also the default when omitted.
    All,
    /// DISTINCT output values.
    Distinct,
    /// DISTINCTROW source records; not interchangeable with DISTINCT.
    DistinctRow,
    /// TOP syntax, including its optional PERCENT modifier.
    Top {
        /// Positive integer count, or percentage when `percent` is true.
        count: u32,
        /// Whether PERCENT was present.
        percent: bool,
    },
}

/// A field projection or wildcard; computed output expressions are deferred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Projection {
    /// All fields, optionally qualified by table name or alias.
    Wildcard(Option<Name>),
    /// A field with an optional explicit AS alias.
    Field {
        /// Field reference.
        field: FieldReference,
        /// Output name.
        alias: Option<Name>,
    },
}

/// A single table name and optional explicit AS alias.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Table {
    /// Source name, without schema or external database qualification.
    pub name: Name,
    /// Local alias; binding is not performed by the parser.
    pub alias: Option<Name>,
}

/// An ORDER BY field and explicit direction (omission normalizes to ascending).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortKey {
    /// Sort field.
    pub field: FieldReference,
    /// DESC when true; ASC otherwise.
    pub descending: bool,
}
