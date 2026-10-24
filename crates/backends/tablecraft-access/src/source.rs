mod collation;
mod collections;
mod multivalue;
mod projection;
mod saved;
mod schema;
pub use saved::{NativeSavedReadError, NativeSavedSelectError};
mod selection;
mod snapshot;

use std::cell::{OnceCell, RefCell};
use tablecraft_ace::{InspectError, TableSnapshot, TableSnapshotLimits, inspect_catalog};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::{CatalogScope, ModelError, ObjectKind};
use tablecraft_query::{
    QueryError,
    source::{ScanConstraint, Source},
};
use tablecraft_schema::Column;

struct Entry {
    name: String,
    columns: OnceCell<schema::ScalarSchema>,
    table: OnceCell<TableSnapshot>,
    index: Option<tablecraft_schema::SchemaName>,
    projected: RefCell<Vec<projection::CachedProjection>>,
    multivalues: OnceCell<multivalue::CollectionCache>,
    attachments: OnceCell<collections::AttachmentCache>,
}
/// One inert catalog and lazily loaded, cached scalar snapshots. No shared-file
/// locking or transaction snapshot is implied: the source must remain unchanged.
/// Binding discovers ordinary scalar columns and explicit unavailable field names
/// without decoding user records. Mixed tables use ordinary scalar projections;
/// unavailable fields remain unsupported for ordinary references and wildcards.
/// Explicit with_multivalues() additionally exposes qualified stored .Value views,
/// cached separately from scalar snapshots and expanded by the query layer.
/// Referenced local saved SELECT/UNION queries are reconstructed lazily into stable owned
/// metadata for the shared query binder. Cached definitions/tables avoid repeat
/// I/O in joins/subqueries; budgets accumulate across catalog/query/schema/table reads. This synchronous source is not a concurrent host.
pub struct NativeSource<'a, R> {
    source: RefCell<R>,
    budget: RefCell<ResourceBudget>,
    scope: CatalogScope,
    snapshot_limits: TableSnapshotLimits,
    cancelled: &'a dyn Fn() -> bool,
    entries: Vec<Entry>,
    queries: Vec<saved::QueryEntry>,
    multivalues: bool,
    collections: bool,
    decoded_attachments: bool,
    auto_indexes: bool,
    text_policy_scope: collation::TextPolicyScope,
}
impl<'a, R: tablecraft_ace::NativeReadSource> NativeSource<'a, R> {
    /// Inventory an explicitly supplied stable source without activating content.
    /// Linked entries can be inventoried but fail if used as query sources.
    pub fn new(
        source: R,
        scope: CatalogScope,
        budget: ResourceBudget,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        Self::new_with_snapshot_limits(
            source,
            scope,
            budget,
            cancelled,
            TableSnapshotLimits::default(),
        )
    }
    /// Inventory with explicit scalar and complete collection-table row limits.
    /// Sequential loads additionally honor the batch size. Complete caches remain
    /// unpublished until EOF and validation. Index structural, per-parent collection
    /// and payload ceilings remain independent; controls never refill between batches.
    pub fn new_with_snapshot_limits(
        mut source: R,
        scope: CatalogScope,
        mut budget: ResourceBudget,
        cancelled: &'a dyn Fn() -> bool,
        snapshot_limits: TableSnapshotLimits,
    ) -> Result<Self, InspectError> {
        let catalog = inspect_catalog(&mut source, scope, &mut budget, cancelled)?;
        let queries = saved::entries(&catalog, &mut budget, cancelled)?;
        let count = catalog
            .catalog()
            .objects()
            .iter()
            .filter(|o| o.kind() == ObjectKind::Table)
            .count();
        budget.charge_allocation(count as u64, std::mem::size_of::<Entry>() as u64)?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;
        for o in catalog
            .catalog()
            .objects()
            .iter()
            .filter(|o| o.kind() == ObjectKind::Table)
        {
            if cancelled() {
                return Err(InspectError::Cancelled);
            }
            let name = o.name().as_str();
            budget.charge_allocation(name.len() as u64, 1)?;
            let mut owned = String::new();
            owned
                .try_reserve_exact(name.len())
                .map_err(|_| InspectError::Allocation)?;
            owned.push_str(name);
            entries.push(Entry {
                name: owned,
                columns: OnceCell::new(),
                table: OnceCell::new(),
                index: None,
                projected: RefCell::new(Vec::new()),
                multivalues: OnceCell::new(),
                attachments: OnceCell::new(),
            });
        }
        Ok(Self {
            source: RefCell::new(source),
            budget: RefCell::new(budget),
            scope,
            snapshot_limits,
            cancelled,
            entries,
            queries,
            multivalues: false,
            collections: false,
            decoded_attachments: false,
            auto_indexes: false,
            text_policy_scope: collation::TextPolicyScope::Unselected,
        })
    }
    /// Opt into qualified scalar multivalue metadata and stored collection reads.
    /// Discovery remains metadata-only; values load only for explicit .Value use.
    /// Lookup queries and active content never run. Expanded reads currently
    /// require sequential parent order and reject a selected index on that table.
    pub fn with_multivalues(mut self) -> Self {
        self.multivalues = true;
        self
    }
    /// Opt into stored multivalues and named attachment metadata members.
    /// Binding reads metadata only. Child snapshots remain complete and bounded;
    /// This option alone keeps payloads encoded; FileData needs a separate opt-in.
    pub fn with_collections(mut self) -> Self {
        self.multivalues = true;
        self.collections = true;
        self
    }
    /// Explicit TableCraft decoded FileData view over qualified attachment members.
    /// Enables collection metadata; does not claim native SQL binary representation.
    /// Only referencing FileData decodes complete bounded payload collections.
    pub fn with_decoded_attachments(mut self) -> Self {
        self.multivalues = true;
        self.collections = true;
        self.decoded_attachments = true;
        self
    }
    /// Opt into metadata-based index selection for safe scalar query constraints.
    /// Unconstrained reads remain sequential. Explicit table indexes take priority.
    /// The first matching qualified single-key definition is selected; a selected
    /// read failure is returned without trying another index or sequential scan.
    /// Complete cached snapshots remain reusable; partial selections are not cached.
    pub fn with_auto_indexes(mut self) -> Self {
        self.auto_indexes = true;
        self
    }

    /// Require a named native index when the selected table is loaded. Other
    /// tables retain sequential reads. Index choice is explicit; safe matching
    /// query constraints may bound its reads. No sequential fallback occurs;
    /// index eligibility is checked on load. Reconfiguration discards that table's
    /// cached snapshot without refunding its already consumed budget.
    pub fn with_index(mut self, table: &str, index: &str) -> Result<Self, InspectError> {
        if (self.cancelled)() {
            return Err(InspectError::Cancelled);
        }
        let mut matches = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.name.eq_ignore_ascii_case(table));
        let (position, _) = matches.next().ok_or(InspectError::UnknownTable)?;
        if matches.next().is_some() {
            return Err(ModelError::AmbiguousName.into());
        }
        self.budget
            .get_mut()
            .charge_allocation(index.len() as u64, 1)?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(index.len())
            .map_err(|_| InspectError::Allocation)?;
        owned.push_str(index);
        let name = tablecraft_schema::SchemaName::new(owned)?;
        let entry = &mut self.entries[position];
        entry.index = Some(name);
        entry.table.take();
        entry.projected.get_mut().clear();
        Ok(self)
    }
    fn entry(&self, name: &str) -> Result<&Entry, QueryError> {
        if (self.cancelled)() {
            return Err(QueryError::Cancelled);
        }
        let mut matches = self
            .entries
            .iter()
            .filter(|e| e.name.eq_ignore_ascii_case(name));
        let entry = matches.next().ok_or(QueryError::UnknownName)?;
        if matches.next().is_some() {
            return Err(QueryError::AmbiguousName);
        }
        Ok(entry)
    }
}
impl<R: tablecraft_ace::NativeReadSource> Source for NativeSource<'_, R> {
    fn query(&self, name: &str) -> Result<Option<&tablecraft_sql::query::Query>, QueryError> {
        self.named_query(name)
    }
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.schema_columns(table)
    }
    fn text_collations(
        &self,
        table: &str,
    ) -> Result<&[Option<tablecraft_query::source::TextCollationId>], QueryError> {
        self.schema_columns(table)?;
        Ok(&self
            .entry(table)?
            .columns
            .get()
            .ok_or(QueryError::Source)?
            .text_collations)
    }
    fn unavailable_columns(
        &self,
        table: &str,
    ) -> Result<&[tablecraft_schema::SchemaName], QueryError> {
        self.schema_columns(table)?;
        Ok(&self
            .entry(table)?
            .columns
            .get()
            .ok_or(QueryError::Source)?
            .unavailable)
    }
    fn multivalue_columns(&self, table: &str) -> Result<&[Column], QueryError> {
        self.collection_columns(table)
    }
    fn multivalue_text_collations(
        &self,
        table: &str,
    ) -> Result<&[Option<tablecraft_query::source::TextCollationId>], QueryError> {
        self.collection_text_collations(table)
    }
    fn collection_members(
        &self,
        table: &str,
    ) -> Result<&[tablecraft_schema::CollectionMember], QueryError> {
        self.attachment_members(table)
    }
    fn collection_member_text_collations(
        &self,
        table: &str,
    ) -> Result<&[Option<tablecraft_query::source::TextCollationId>], QueryError> {
        self.attachment_text_collations(table)
    }
    fn scan_collection_members(
        &self,
        table: &str,
        indices: &[usize],
        values: &[usize],
        members: &[usize],
        visitor: &mut tablecraft_query::source::MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        self.member_scan(table, indices, values, members, visitor)
    }
    fn scan_multivalues(
        &self,
        table: &str,
        indices: &[usize],
        collections: &[usize],
        visitor: &mut tablecraft_query::source::MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        self.member_scan(table, indices, collections, &[], visitor)
    }
    fn scan(
        &self,
        table: &str,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        for row in self.table(table)?.rows() {
            if (self.cancelled)() {
                return Err(QueryError::Cancelled);
            }
            visitor(row)?;
        }
        Ok(())
    }
    fn scan_projected(
        &self,
        table: &str,
        indices: &[usize],
        constraint: Option<ScanConstraint>,
        visitor: &mut dyn FnMut(
            tablecraft_query::source::ProjectedRow<'_>,
        ) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.projected_scan(table, indices, constraint, visitor)
    }
    fn scan_constrained(
        &self,
        table: &str,
        constraint: ScanConstraint,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        self.selected_scan(table, constraint, visitor)
    }
}
fn query_error(error: InspectError) -> QueryError {
    match error {
        InspectError::Unsupported | InspectError::Encoded => QueryError::Unsupported,
        InspectError::UnknownTable | InspectError::UnknownIndex => QueryError::UnknownName,
        InspectError::Cancelled => QueryError::Cancelled,
        InspectError::StructureLimit => QueryError::StructureLimit,
        InspectError::Allocation | InspectError::Model(ModelError::Allocation) => {
            QueryError::Allocation
        }
        InspectError::Model(ModelError::AmbiguousName) => QueryError::AmbiguousName,
        InspectError::Schema(error) => QueryError::Schema(error),
        InspectError::Limit(error) => QueryError::Limit(error),
        _ => QueryError::Source,
    }
}
