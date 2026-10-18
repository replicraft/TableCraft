//! Owned Access query statements. Parsing never opens or changes a database.
mod actions;
mod format;
mod parameters;
mod read;
pub use actions::{Assignment, Delete, Insert, InsertSource, Update};
pub use parameters::{Parameter, ParameterType};
pub use read::{Crosstab, ReadBody, ReadQuery, UnionArm};
/// One query with optional parameter declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    /// Declared parameters in written order.
    pub parameters: Vec<Parameter>,
    /// Query statement.
    pub statement: Statement,
}
/// Access data-query forms; DDL and pass-through SQL are separate facilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    /// SELECT/TABLE and UNION queries.
    Read(Box<ReadQuery>),
    /// SELECT INTO make-table query, never admitted as a retrieval subquery.
    MakeTable(Box<crate::select::Select>),
    /// TRANSFORM/SELECT/PIVOT crosstab query.
    Crosstab(Box<Crosstab>),
    /// INSERT INTO append query.
    Insert(Box<Insert>),
    /// UPDATE query.
    Update(Box<Update>),
    /// DELETE query.
    Delete(Box<Delete>),
}
