//! TableCraft-owned syntax for the SELECT expressions, clauses and relations.

mod expression;
pub(crate) mod format;
mod name;
mod source;

pub use expression::{BinaryOperator, Expression, FunctionArgument, Quantifier, UnaryOperator};
pub use name::{FieldReference, Name};
pub use source::{JoinKind, TableSource};

/// A SELECT syntax model, without binding, coercion or execution semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Select {
    /// Row selection syntax. TOP is not a promise of LIMIT-like execution.
    pub predicate: SelectPredicate,
    /// Ordered output columns.
    pub projection: Vec<Projection>,
    /// Sources in written order; joins retain their nested structure.
    pub from: Vec<TableSource>,
    /// GROUP BY expressions.
    pub group_by: Vec<Expression>,
    /// HAVING expression.
    pub having: Option<Expression>,
    /// Optional make-table destination.
    pub into: Option<Name>,
    /// Optional external make-table destination, distinct from a source IN clause.
    pub into_external: Option<ExternalDatabase>,
    /// External source database and optional connection text from IN.
    pub external: Option<ExternalDatabase>,
    /// Whether WITH OWNERACCESS OPTION was specified.
    pub owner_access: bool,
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
    /// DISTINCT combined with TOP.
    DistinctTop {
        /// Written count.
        count: u32,
        /// PERCENT modifier.
        percent: bool,
    },
    /// DISTINCTROW combined with TOP.
    DistinctRowTop {
        /// Written count.
        count: u32,
        /// PERCENT modifier.
        percent: bool,
    },
    /// TOP syntax, including its optional PERCENT modifier.
    Top {
        /// Positive integer count, or percentage when `percent` is true.
        count: u32,
        /// Whether PERCENT was present.
        percent: bool,
    },
}

/// An ordered output projection: field, wildcard or computed expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Projection {
    /// Computed projection, optionally named with AS.
    Expression {
        /// Output expression.
        expression: Expression,
        /// Explicit output alias.
        alias: Option<Name>,
    },
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

/// An ORDER BY expression and explicit direction (omission normalizes to ascending).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortKey {
    /// Sort expression.
    pub expression: Expression,
    /// DESC when true; ASC otherwise.
    pub descending: bool,
}

/// Explicit external source; these strings are never opened by the parser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalDatabase {
    /// Database path or connection specification.
    pub database: String,
    /// Optional connection string.
    pub connection: Option<String>,
    /// Whether the connection was bracket-delimited rather than quoted text.
    pub connection_bracketed: bool,
}
