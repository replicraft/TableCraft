mod projection;
mod saved;
mod schema;
pub use saved::NativeSavedSelectError;
mod selection;
mod snapshot;

use std::cell::{OnceCell, RefCell};
use tablecraft_ace::{InspectError, TableSnapshot, inspect_catalog};
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
}
/// One inert catalog and lazily loaded, cached scalar snapshots. No shared-file
/// locking or transaction snapshot is implied: the source must remain unchanged.
/// Binding discovers ordinary scalar columns and explicit unavailable field names
/// without decoding user records. Mixed tables use ordinary scalar projections;
/// unavailable fields remain unsupported for references and wildcard expansion.
/// Referenced local saved SELECTs are reconstructed lazily into stable owned
/// metadata for the shared query binder. Cached definitions/tables avoid repeat
/// I/O in joins/subqueries; budgets accumulate across catalog/query/schema/table reads. This synchronous source is not a concurrent host.
pub struct NativeSource<'a, R> {
    source: RefCell<R>,
    budget: RefCell<ResourceBudget>,
    scope: CatalogScope,
    cancelled: &'a dyn Fn() -> bool,
    entries: Vec<Entry>,
    queries: Vec<saved::QueryEntry>,
}
impl<'a, R: tablecraft_ace::NativeReadSource> NativeSource<'a, R> {
    /// Inventory an explicitly supplied stable source without activating content.
    /// Linked entries can be inventoried but fail if used as query sources.
    pub fn new(
        mut source: R,
        scope: CatalogScope,
        mut budget: ResourceBudget,
        cancelled: &'a dyn Fn() -> bool,
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
            });
        }
        Ok(Self {
            source: RefCell::new(source),
            budget: RefCell::new(budget),
            scope,
            cancelled,
            entries,
            queries,
        })
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
