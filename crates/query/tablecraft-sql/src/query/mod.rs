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

impl Query {
    /// Wrap already-owned SELECT syntax, moving final ordering to the read query.
    ///
    /// This performs no parsing, validation, binding or execution. The producer
    /// remains responsible for its syntax and allocation budgets.
    pub fn from_select(mut select: crate::select::Select) -> Self {
        let order_by = std::mem::take(&mut select.order_by);
        let owner_access = std::mem::replace(&mut select.owner_access, false);
        Self {
            parameters: Vec::new(),
            statement: Statement::Read(Box::new(ReadQuery {
                body: ReadBody::Select(Box::new(select)),
                order_by,
                owner_access,
            })),
        }
    }

    /// Wrap already-owned crosstab syntax without parsing or validating it.
    /// The producer remains responsible for its syntax and allocation budgets.
    pub fn from_crosstab(crosstab: Crosstab) -> Self {
        Self {
            parameters: Vec::new(),
            statement: Statement::Crosstab(Box::new(crosstab)),
        }
    }
}
