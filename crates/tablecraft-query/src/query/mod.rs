//! Owned Access query statements. Parsing never opens or changes a database.
mod actions;
mod data_type;
pub mod definition;
mod format;
mod parameters;
mod read;
pub use actions::{Assignment, Delete, Insert, InsertSource, Update};
pub use data_type::DataType;
/// Type syntax used by a parameter declaration.
pub type ParameterType = DataType;
pub use parameters::Parameter;
pub use read::{Crosstab, ReadBody, ReadQuery, UnionArm};
/// One query with optional parameter declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    /// Declared parameters in written order.
    pub parameters: Vec<Parameter>,
    /// Query statement.
    pub statement: Statement,
}
/// Access query forms; pass-through SQL is a separate facility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Statement {
    /// Data-definition syntax, without applying schema changes.
    Definition(Box<definition::Definition>),
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
