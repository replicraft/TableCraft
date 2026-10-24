//! Read-only source contracts and a validated in-memory implementation.
mod collation;
pub use collation::{CollationResolver, TextCollationId, TextPolicy};
pub(crate) use collation::{
    validate as validate_text_collations, validate_types as validate_text_policy_types,
};
mod constraint;
mod multivalue;
pub use multivalue::{MultivalueList, MultivalueVisitor};
mod projection;
pub use constraint::ScanConstraint;
pub use projection::{ProjectedRow, validate_projection};

use crate::{QueryError, profile::SOURCE_SCHEMA_POLICY};
use tablecraft_core::values::Value;
use tablecraft_schema::{Column, SchemaName, validate_row};

/// Synchronous, borrowed-row source. Scan callbacks must propagate rejection.
/// Hosts own I/O cancellation/limits before invoking the visitor; the executor
/// bounds every visited row. No database or provider is opened implicitly.
pub trait Source {
    /// Optional named retrieval definition in the same source namespace as tables.
    /// Return None for ordinary/unknown table names. Providers must reject ambiguous
    /// table/query declarations and return stable owned syntax borrowed from their
    /// metadata cache, without reading user rows or activating providers/functions.
    /// Preparation captures this definition in its owned plan; a changed definition
    /// requires preparing again. The default exposes no named queries.
    fn query(&self, _name: &str) -> Result<Option<&tablecraft_sql::query::Query>, QueryError> {
        Ok(None)
    }
    /// Return stable available scalar columns, defining the exact runtime row order.
    fn columns(&self, table: &str) -> Result<&[Column], QueryError>;
    /// Stable opaque policy identities aligned with available scalar columns.
    /// Empty means no per-column policy metadata is supplied. Otherwise the
    /// length must equal columns(); only Text fields may have Some identity.
    /// None declares no explicit field policy. Discovery must not scan user rows,
    /// open providers or infer a comparer. Unknown host policies may retain an
    /// identity; identity alone never qualifies or activates their semantics.
    /// Preparation captures this metadata and execution checks it for changes,
    /// even for unprojected fields. Existing explicit execution policies remain
    /// unchanged unless the host explicitly supplies an execution resolver.
    fn text_collations(&self, _table: &str) -> Result<&[Option<TextCollationId>], QueryError> {
        Ok(&[])
    }

    /// Names present in the stable source schema but unavailable as scalar values.
    /// They must be disjoint from columns(), unique under the source name policy,
    /// and remain stable through execution. Ordinary references and matching wildcards
    /// fail explicitly rather than silently omitting fields or manufacturing NULL.
    /// Rows contain exactly columns(), in that order, including zero-width rows
    /// when all declared fields are unavailable. Existing scalar-only sources
    /// retain the default empty list. This method must not scan source records.
    fn unavailable_columns(&self, _table: &str) -> Result<&[SchemaName], QueryError> {
        Ok(&[])
    }
    /// Stable qualified scalar collections. Each column name must also occur in
    /// unavailable_columns(), and describes stored element type/nullability, not
    /// a scalar parent field. Metadata discovery must not scan source records.
    /// The default exposes no collection access.
    fn multivalue_columns(&self, _table: &str) -> Result<&[Column], QueryError> {
        Ok(&[])
    }
    /// Stable policies aligned with multivalue_columns(), under the same rules
    /// as text_collations(). Only stored Text elements may carry an identity.
    fn multivalue_text_collations(
        &self,
        _table: &str,
    ) -> Result<&[Option<TextCollationId>], QueryError> {
        Ok(&[])
    }
    /// Stable named scalar members of aligned child records. Each collection
    /// name also occurs in unavailable_columns(); member pairs must be unique.
    /// Collection names must not overlap multivalue_columns(). Metadata discovery
    /// reads no user records. The default exposes no named members.
    /// A bare named collection's WHERE IS NULL/IS NOT NULL criteria test empty/
    /// nonempty membership. NULL child values still count as members. A selected
    /// representative member view supplies the complete validated membership;
    /// this does not expose the bare collection as a scalar or a display value.
    fn collection_members(
        &self,
        _table: &str,
    ) -> Result<&[tablecraft_schema::CollectionMember], QueryError> {
        Ok(&[])
    }
    /// Stable policies aligned with collection_members(), under the same rules
    /// as text_collations(). Only Text members may carry an identity.
    fn collection_member_text_collations(
        &self,
        _table: &str,
    ) -> Result<&[Option<TextCollationId>], QueryError> {
        Ok(&[])
    }
    /// Visit parent projections, scalar multivalue lists, then named member lists
    /// in requested order. Members of one collection must have equal length and
    /// identical child order. The query layer aligns them before taking products
    /// across different collections. The default bridges existing value providers.
    fn scan_collection_members(
        &self,
        table: &str,
        indices: &[usize],
        values: &[usize],
        members: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        if !members.is_empty() {
            return Err(QueryError::Unsupported);
        }
        self.scan_multivalues(table, indices, values, visitor)
    }
    /// Visit parent scalar projections and requested collection lists together.
    /// Requests are sorted unique positions in columns()/multivalue_columns().
    /// One callback per parent, including parents with empty collections; lists
    /// follow requested order. Stop on rejection and never manufacture scalar
    /// placeholders for unavailable fields. The query layer owns row expansion.
    fn scan_multivalues(
        &self,
        _table: &str,
        _indices: &[usize],
        _collections: &[usize],
        _visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        Err(QueryError::Unsupported)
    }
    /// Visit rows in source order, stopping immediately on visitor failure.
    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError>;
    /// Visit explicitly requested scalar columns in sorted unique source order.
    /// Constraint positions still refer to the complete columns() schema. Hosts
    /// may avoid decoding unrequested payloads; unavailable fields are not cells.
    /// The default validates complete supplied rows and borrows selected cells,
    /// retaining existing scan/constraint and visitor-failure behavior.
    fn scan_projected(
        &self,
        table: &str,
        indices: &[usize],
        constraint: Option<ScanConstraint>,
        visitor: &mut dyn FnMut(ProjectedRow<'_>) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        let columns = self.columns(table)?;
        validate_projection(indices, columns.len())?;
        let mut failure: Option<QueryError> = None;
        let mut select = |row: &[Value]| {
            if let Some(error) = &failure {
                return Err(error.clone());
            }
            let result = (|| {
                validate_row(columns, row)?;
                visitor(ProjectedRow::select(row, indices)?)
            })();
            if let Err(error) = &result {
                failure = Some(error.clone());
            }
            result
        };
        let result = if let Some(constraint) = constraint {
            self.scan_constrained(table, constraint, &mut select)
        } else {
            self.scan(table, &mut select)
        };
        failure.map_or(result, Err)
    }
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
