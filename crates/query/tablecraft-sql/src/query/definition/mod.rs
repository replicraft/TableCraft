//! Data-definition syntax, without database or schema mutation.
mod constraint;
mod format;
mod index;
mod saved;
mod table;
use crate::select::Name;
pub use constraint::{Constraint, ConstraintKind, Reference, ReferentialAction};
pub use index::{CreateIndex, IndexField, IndexPolicy};
pub use saved::{CreateView, Procedure};
pub use table::{AlterOperation, AlterTable, Column, CreateTable, TableElement};

/// A schema statement. Catalog existence and field semantics are not checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Definition {
    /// CREATE VIEW.
    CreateView(CreateView),
    /// CREATE PROCEDURE or a PROCEDURE query declaration.
    Procedure(Procedure),
    /// DROP VIEW.
    DropView(Name),
    /// DROP PROCEDURE.
    DropProcedure(Name),
    /// CREATE TABLE.
    CreateTable(CreateTable),
    /// ALTER TABLE, one operation at a time.
    AlterTable(AlterTable),
    /// CREATE INDEX.
    CreateIndex(CreateIndex),
    /// DROP TABLE.
    DropTable(Name),
    /// DROP INDEX, with its owning table.
    DropIndex {
        /// Index name.
        index: Name,
        /// Owning table name.
        table: Name,
    },
}
