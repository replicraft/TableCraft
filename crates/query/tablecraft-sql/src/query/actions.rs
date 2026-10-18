use super::ReadQuery;
use crate::select::{Expression, ExternalDatabase, FieldReference, Name, TableSource};
/// INSERT INTO append syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Insert {
    /// Destination table.
    pub target: Name,
    /// Explicit field list, or empty when omitted.
    pub fields: Vec<Name>,
    /// Optional external destination.
    pub external: Option<ExternalDatabase>,
    /// Values or retrieval query.
    pub source: InsertSource,
}
/// Append data source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertSource {
    /// A single VALUES record; Access multi-record appends use SELECT.
    Values(Vec<Expression>),
    /// Retrieval source.
    Query(Box<ReadQuery>),
}
/// UPDATE assignment, without name resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Assignment {
    /// Field receiving the value.
    pub target: FieldReference,
    /// Assigned expression.
    pub value: Expression,
}
/// UPDATE syntax; no mutation occurs while parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    /// Source relations, possibly joined.
    pub sources: Vec<TableSource>,
    /// Ordered SET assignments.
    pub assignments: Vec<Assignment>,
    /// Optional WHERE.
    pub selection: Option<Expression>,
}
/// DELETE syntax; selected targets are retained without deciding updateability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delete {
    /// Optional target table wildcards (empty for DELETE FROM or DELETE * FROM).
    pub targets: Vec<Name>,
    /// Source relations.
    pub sources: Vec<TableSource>,
    /// Optional WHERE.
    pub selection: Option<Expression>,
}
