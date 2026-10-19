//! Read-only source contracts and a validated in-memory implementation.
mod constraint;
pub use constraint::ScanConstraint;

use crate::{QueryError, profile::SOURCE_SCHEMA_POLICY};
use tablecraft_core::values::Value;
use tablecraft_schema::{Column, SchemaName, validate_row};

/// Synchronous, borrowed-row source. Scan callbacks must propagate rejection.
/// Hosts own I/O cancellation/limits before invoking the visitor; the executor
/// bounds every visited row. No database or provider is opened implicitly.
pub trait Source {
    /// Return stable available scalar columns, defining the exact runtime row order.
    fn columns(&self, table: &str) -> Result<&[Column], QueryError>;
    /// Names present in the stable source schema but unavailable as scalar values.
    /// They must be disjoint from columns(), unique under the source name policy,
    /// and remain stable through execution. References and matching wildcards
    /// fail explicitly rather than silently omitting fields or manufacturing NULL.
    /// Rows contain exactly columns(), in that order, including zero-width rows
    /// when all declared fields are unavailable. Existing scalar-only sources
    /// retain the default empty list. This method must not scan source records.
    fn unavailable_columns(&self, _table: &str) -> Result<&[SchemaName], QueryError> {
        Ok(&[])
    }
    /// Visit rows in source order, stopping immediately on visitor failure.
    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError>;
    /// Visit a superset of rows matching a proven-safe constraint. Implementors
    /// may ignore the constraint; the executor always retains its full filter.
    /// Never omit matching rows, change their carriers/order, hide source errors
    /// in visited data, or ignore visitor rejection. Constraint columns address
    /// dense columns() positions, never unavailable names. The default scans all rows.
    fn scan_constrained(
        &self,
        table: &str,
        _constraint: ScanConstraint,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.scan(table, visitor)
    }
}
/// Validated owned relation for tests and explicitly supplied small snapshots.
/// Diagnostic formatting deliberately does not expose records or names.
pub struct MemoryTable {
    name: SchemaName,
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
        let name = SchemaName::new(name)?;
        SOURCE_SCHEMA_POLICY.validate_name(&name)?;
        SOURCE_SCHEMA_POLICY.validate_columns(&columns)?;
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
            if tables[..i].iter().any(|other| {
                SOURCE_SCHEMA_POLICY
                    .names
                    .equals(other.name.as_str(), t.name.as_str())
            }) {
                return Err(QueryError::AmbiguousName);
            }
        }
        Ok(Self { tables })
    }
    fn table(&self, name: &str) -> Result<&MemoryTable, QueryError> {
        self.tables
            .iter()
            .find(|t| SOURCE_SCHEMA_POLICY.names.equals(t.name.as_str(), name))
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
