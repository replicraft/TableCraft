use crate::{
    query::{Parameter, Query, ReadQuery},
    select::Name,
};
/// CREATE VIEW, retaining optional output field names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateView {
    /// View name.
    pub name: Name,
    /// Optional ordered output names; matching the projection requires binding.
    pub fields: Vec<Name>,
    /// Retrieval query; make-table syntax is excluded.
    pub query: ReadQuery,
}
/// Stored procedure/query definition, containing one SQL body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Procedure {
    /// Stored name.
    pub name: Name,
    /// Ordered parameter declarations.
    pub parameters: Vec<Parameter>,
    /// CREATE PROCEDURE when true, PROCEDURE declaration otherwise.
    pub create: bool,
    /// AS when true, semicolon header terminator otherwise.
    pub as_separator: bool,
    /// One body, which is parsed but never registered or executed.
    pub body: Box<Query>,
}
