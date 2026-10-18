use super::Constraint;
use crate::{query::DataType, select::Name};
/// CREATE TABLE with field and constraint order retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateTable {
    /// Table name.
    pub name: Name,
    /// Session-scoped TEMPORARY syntax.
    pub temporary: bool,
    /// Written definitions, containing at least one column when parsed.
    pub elements: Vec<TableElement>,
}
/// One item inside a table definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableElement {
    /// Field definition.
    Column(Column),
    /// Table-level named constraint.
    Constraint(Constraint),
}
/// Field syntax, without storage layout or schema validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    /// Field name.
    pub name: Name,
    /// Exact recognized type name and integer arguments.
    pub data_type: DataType,
    /// Explicit NOT NULL.
    pub not_null: bool,
    /// WITH COMPRESSION (including its COMP spelling).
    pub compression: bool,
    /// Inline constraint on this field.
    pub constraint: Option<Constraint>,
}
/// ALTER TABLE and its single operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlterTable {
    /// Existing table name.
    pub name: Name,
    /// One column or constraint operation.
    pub operation: AlterOperation,
}
/// Supported Access ALTER TABLE operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlterOperation {
    /// ADD COLUMN.
    AddColumn(Column),
    /// ALTER COLUMN, changing the type annotation.
    AlterColumn {
        /// Field name.
        name: Name,
        /// New syntactic type annotation.
        data_type: DataType,
    },
    /// ADD CONSTRAINT.
    AddConstraint(Constraint),
    /// DROP COLUMN.
    DropColumn(Name),
    /// DROP CONSTRAINT.
    DropConstraint(Name),
}
