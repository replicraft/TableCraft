//! Read-only source contracts and a validated in-memory implementation.
use crate::QueryError;
use tablecraft_core::values::Value;
use tablecraft_schema::{Column, validate_columns, validate_row};

/// Synchronous, borrowed-row source. Scan callbacks must propagate rejection.
/// Hosts own I/O cancellation/limits before invoking the visitor; the executor
/// bounds every visited row. No database or provider is opened implicitly.
pub trait Source {
    /// Return a table's stable schema or a typed lookup failure.
    fn columns(&self, table: &str) -> Result<&[Column], QueryError>;
    /// Visit rows in source order, stopping immediately on visitor failure.
    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError>;
}
/// Validated owned relation for tests and explicitly supplied small snapshots.
/// Diagnostic formatting deliberately does not expose records or names.
pub struct MemoryTable {
    name: String,
    columns: Vec<Column>,
    rows: Vec<Vec<Value>>,
}
impl MemoryTable {
    /// Validate a table and its exact carriers/nullability without coercion.
    pub fn new(
        name: String,
        columns: Vec<Column>,
        rows: Vec<Vec<Value>>,
    ) -> Result<Self, QueryError> {
        if name.is_empty() || name.len() > 255 || name.chars().any(char::is_control) {
            return Err(tablecraft_schema::SchemaError::Name.into());
        }
        validate_columns(&columns)?;
        for row in &rows {
            validate_row(&columns, row)?;
        }
        Ok(Self {
            name,
            columns,
            rows,
        })
    }
}
/// Explicit in-memory catalog. Names use ASCII-insensitive matching; non-ASCII
/// spelling is exact. This is not an Access locale identifier collation.
#[derive(Default)]
pub struct MemorySource {
    tables: Vec<MemoryTable>,
}
impl MemorySource {
    /// Reject duplicate table identities.
    pub fn new(tables: Vec<MemoryTable>) -> Result<Self, QueryError> {
        for (i, t) in tables.iter().enumerate() {
            if tables[..i]
                .iter()
                .any(|other| other.name.eq_ignore_ascii_case(&t.name))
            {
                return Err(QueryError::AmbiguousName);
            }
        }
        Ok(Self { tables })
    }
    fn table(&self, name: &str) -> Result<&MemoryTable, QueryError> {
        self.tables
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
            .ok_or(QueryError::UnknownName)
    }
}
impl Source for MemorySource {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        Ok(&self.table(table)?.columns)
    }
    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        for row in &self.table(table)?.rows {
            visitor(row)?;
        }
        Ok(())
    }
}
