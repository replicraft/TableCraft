use thiserror::Error;

/// Query errors omit source names, SQL and value payloads.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum QueryError {
    /// Feature outside the supported retrieval profile.
    #[error("query feature is not implemented")]
    Unsupported,
    /// Distinct field text policies contribute to a text comparison without
    /// an established precedence rule under per-operation resolution.
    #[error("conflicting text comparison policies")]
    CollationConflict,
    /// The selected operation requires unavailable Scalar/Variant provenance.
    #[error("required expression declaration is unknown")]
    ArithmeticDeclaration,
    /// Missing table, field or declared parameter.
    #[error("query name has no binding")]
    UnknownName,
    /// Multiple bindings match a name.
    #[error("query name is ambiguous")]
    AmbiguousName,
    /// A named source depends on itself through the current dependency path.
    #[error("query source dependency cycle")]
    DependencyCycle,
    /// Parameter list or supplied types disagree with declarations.
    #[error("query parameter mismatch")]
    Parameter,
    /// Source schema changed after preparation.
    #[error("source schema changed after preparation")]
    SchemaChanged,
    /// Invalid grouping or aggregate usage.
    #[error("invalid grouped expression")]
    Grouping,
    /// Retrieval branches differ in column count.
    #[error("incompatible query result width")]
    Width,
    /// Scalar subquery returned multiple rows.
    #[error("scalar subquery returned multiple rows")]
    Cardinality,
    /// Configured hard ceiling exceeded.
    #[error("query structural limit exceeded")]
    StructureLimit,
    /// Host cancelled the operation.
    #[error("query execution cancelled")]
    Cancelled,
    /// Allocation failed after checking the budget.
    #[error("query allocation failed")]
    Allocation,
    /// Host source failed without exposing its private data.
    #[error("query source failed")]
    Source,
    /// Invalid source schema/row.
    #[error(transparent)]
    Schema(#[from] tablecraft_schema::SchemaError),
    /// Consumable operation limit.
    #[error(transparent)]
    Limit(#[from] tablecraft_core::limits::LimitError),
    /// Scalar failure.
    #[error(transparent)]
    Evaluation(#[from] tablecraft_expressions::EvalError),
}

impl From<tablecraft_db::ProjectionError> for QueryError {
    fn from(_: tablecraft_db::ProjectionError) -> Self {
        Self::Source
    }
}
