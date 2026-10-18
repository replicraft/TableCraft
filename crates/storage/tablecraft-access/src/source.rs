use std::{
    cell::{OnceCell, RefCell},
    io::{Read, Seek},
};
use tablecraft_ace::{InspectError, TableSnapshot, inspect_catalog, read_table};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_model::{CatalogScope, ModelError, ObjectKind};
use tablecraft_query::{QueryError, source::Source};
use tablecraft_schema::Column;

struct Entry {
    name: String,
    table: OnceCell<TableSnapshot>,
}
/// One inert catalog and lazily loaded, cached scalar snapshots. No shared-file
/// locking or transaction snapshot is implied: the source must remain unchanged.
/// All columns/rows of a referenced table are read before binding can finish.
/// Cached tables avoid repeat I/O in joins/subqueries; budgets accumulate across
/// catalog/schema/table reads. This synchronous source is not a concurrent host.
pub struct NativeSource<'a, R> {
    source: RefCell<R>,
    budget: RefCell<ResourceBudget>,
    scope: CatalogScope,
    cancelled: &'a dyn Fn() -> bool,
    entries: Vec<Entry>,
}
impl<'a, R: Read + Seek> NativeSource<'a, R> {
    /// Inventory an explicitly supplied stable source without activating content.
    /// Linked entries can be inventoried but fail if used as query sources.
    pub fn new(
        mut source: R,
        scope: CatalogScope,
        mut budget: ResourceBudget,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Result<Self, InspectError> {
        let catalog = inspect_catalog(&mut source, scope, &mut budget, cancelled)?;
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
            entries.push(Entry {
                name: name.to_owned(),
                table: OnceCell::new(),
            });
        }
        Ok(Self {
            source: RefCell::new(source),
            budget: RefCell::new(budget),
            scope,
            cancelled,
            entries,
        })
    }
    fn table(&self, name: &str) -> Result<&TableSnapshot, QueryError> {
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
        if entry.table.get().is_none() {
            let mut source = self
                .source
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            let mut budget = self
                .budget
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            let table = read_table(
                &mut *source,
                self.scope,
                &entry.name,
                &mut budget,
                self.cancelled,
            )
            .map_err(query_error)?;
            entry.table.set(table).map_err(|_| QueryError::Source)?;
        }
        entry.table.get().ok_or(QueryError::Source)
    }
}
impl<R: Read + Seek> Source for NativeSource<'_, R> {
    fn columns(&self, table: &str) -> Result<&[Column], QueryError> {
        Ok(self.table(table)?.columns())
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
}
fn query_error(error: InspectError) -> QueryError {
    match error {
        InspectError::Unsupported | InspectError::Encoded => QueryError::Unsupported,
        InspectError::UnknownTable => QueryError::UnknownName,
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
