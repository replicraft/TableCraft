//! FROM relations and explicit join structure.
use super::{Expression, Name, Table};
/// A table, nested join, or derived SELECT source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableSource {
    /// Derived retrieval query with an optional alias.
    Derived {
        /// Nested query.
        query: Box<crate::query::ReadQuery>,
        /// Explicit alias.
        alias: Option<Name>,
    },
    /// Named table/query.
    Table(Table),
    /// A parenthesized relation group with an optional alias.
    Group {
        /// Relation inside parentheses.
        source: Box<TableSource>,
        /// Explicit alias.
        alias: Option<Name>,
    },
    /// Explicit join tree, preserving left/right orientation.
    Join {
        /// Left source.
        left: Box<TableSource>,
        /// Join kind.
        kind: JoinKind,
        /// Right source.
        right: Box<TableSource>,
        /// ON condition, without resolution or evaluation.
        condition: Expression,
    },
}
/// Access join kinds; CROSS is represented by comma-separated FROM sources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinKind {
    /// INNER JOIN.
    Inner,
    /// `LEFT [OUTER] JOIN`.
    Left,
    /// `RIGHT [OUTER] JOIN`.
    Right,
}
