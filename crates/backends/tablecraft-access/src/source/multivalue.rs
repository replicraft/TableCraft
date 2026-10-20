//! Complete native collection caches, separate from scalar/index projections.
use super::{NativeSource, query_error};
use std::cell::OnceCell;
use tablecraft_ace::{MultivalueColumn, inspect_multivalue_columns, read_multivalue_column};
use tablecraft_core::limits::Resource;
use tablecraft_query::{
    QueryError,
    source::{MultivalueList, MultivalueVisitor, validate_projection},
};
use tablecraft_schema::Column;

pub(super) struct CollectionCache {
    columns: Vec<Column>,
    snapshots: Vec<OnceCell<MultivalueColumn>>,
}
struct NativeList<'a> {
    column: &'a MultivalueColumn,
    parent: usize,
    length: usize,
}
impl MultivalueList for NativeList<'_> {
    fn len(&self) -> usize {
        self.length
    }
    fn get(&self, index: usize) -> Option<&tablecraft_core::values::Value> {
        self.column.value(self.parent, index)
    }
}
impl<R: tablecraft_ace::NativeReadSource> NativeSource<'_, R> {
    pub(super) fn collection_columns(&self, table: &str) -> Result<&[Column], QueryError> {
        if !self.multivalues {
            return Ok(&[]);
        }
        self.schema_columns(table)?;
        let entry = self.entry(table)?;
        if entry.multivalues.get().is_none() {
            let mut budget = self
                .budget
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            let columns = if entry
                .columns
                .get()
                .ok_or(QueryError::Source)?
                .unavailable
                .is_empty()
            {
                Vec::new()
            } else {
                let mut source = self
                    .source
                    .try_borrow_mut()
                    .map_err(|_| QueryError::Source)?;
                inspect_multivalue_columns(
                    &mut *source,
                    self.scope,
                    &entry.name,
                    &mut budget,
                    self.cancelled,
                )
                .map_err(query_error)?
            };
            if columns.iter().any(|column| {
                !entry.columns.get().is_some_and(|schema| {
                    schema
                        .unavailable
                        .iter()
                        .any(|name| name.as_str().eq_ignore_ascii_case(column.name()))
                })
            }) {
                return Err(QueryError::SchemaChanged);
            }
            budget
                .charge_allocation(
                    columns.len() as u64,
                    size_of::<OnceCell<MultivalueColumn>>() as u64,
                )
                .map_err(|error| query_error(error.into()))?;
            let mut snapshots = Vec::new();
            snapshots
                .try_reserve_exact(columns.len())
                .map_err(|_| QueryError::Allocation)?;
            snapshots.extend((0..columns.len()).map(|_| OnceCell::new()));
            entry
                .multivalues
                .set(CollectionCache { columns, snapshots })
                .map_err(|_| QueryError::Source)?;
        }
        Ok(&entry.multivalues.get().ok_or(QueryError::Source)?.columns)
    }
    pub(super) fn collection_scan(
        &self,
        table: &str,
        indices: &[usize],
        collections: &[usize],
        visitor: &mut MultivalueVisitor<'_>,
    ) -> Result<(), QueryError> {
        let _span = tracing::debug_span!("native.multivalue.scan").entered();
        if !self.multivalues || collections.is_empty() {
            return Err(QueryError::Unsupported);
        }
        let columns = self.schema_columns(table)?;
        validate_projection(indices, columns.len())?;
        let values = self.collection_columns(table)?;
        validate_projection(collections, values.len())?;
        let entry = self.entry(table)?;
        if entry.index.is_some() {
            return Err(QueryError::Unsupported);
        }
        let cache = entry.multivalues.get().ok_or(QueryError::Source)?;
        for index in collections {
            if (self.cancelled)() {
                return Err(QueryError::Cancelled);
            }
            let cell = cache.snapshots.get(*index).ok_or(QueryError::Source)?;
            if cell.get().is_none() {
                let mut source = self
                    .source
                    .try_borrow_mut()
                    .map_err(|_| QueryError::Source)?;
                let mut budget = self
                    .budget
                    .try_borrow_mut()
                    .map_err(|_| QueryError::Source)?;
                let snapshot = read_multivalue_column(
                    &mut *source,
                    self.scope,
                    &entry.name,
                    values[*index].name(),
                    &mut budget,
                    self.cancelled,
                )
                .map_err(query_error)?;
                if snapshot.value_column().type_descriptor() != values[*index].type_descriptor()
                    || snapshot.value_column().nullable() != values[*index].nullable()
                {
                    return Err(QueryError::SchemaChanged);
                }
                cell.set(snapshot).map_err(|_| QueryError::Source)?;
            }
        }
        let parents = cache.snapshots[collections[0]]
            .get()
            .ok_or(QueryError::Source)?
            .links()
            .parents()
            .len();
        if collections.iter().any(|index| {
            cache.snapshots[*index]
                .get()
                .is_none_or(|snapshot| snapshot.links().parents().len() != parents)
        }) {
            return Err(QueryError::Source);
        }
        let mut ordinal = 0;
        self.projected_scan(table, indices, None, &mut |row| {
            if (self.cancelled)() {
                return Err(QueryError::Cancelled);
            }
            if ordinal >= parents {
                return Err(QueryError::Source);
            }
            let mut budget = self
                .budget
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            budget
                .charge(Resource::Steps, collections.len() as u64 + 1)
                .map_err(|error| query_error(error.into()))?;
            budget
                .charge_allocation(
                    collections.len() as u64,
                    size_of::<NativeList<'_>>() as u64 + size_of::<&dyn MultivalueList>() as u64,
                )
                .map_err(|error| query_error(error.into()))?;
            let mut lists = Vec::new();
            lists
                .try_reserve_exact(collections.len())
                .map_err(|_| QueryError::Allocation)?;
            for index in collections {
                let column = cache.snapshots[*index].get().ok_or(QueryError::Source)?;
                lists.push(NativeList {
                    column,
                    parent: ordinal,
                    length: column
                        .links()
                        .parents()
                        .get(ordinal)
                        .ok_or(QueryError::Source)?
                        .child_count(),
                });
            }
            let mut views = Vec::new();
            views
                .try_reserve_exact(lists.len())
                .map_err(|_| QueryError::Allocation)?;
            views.extend(lists.iter().map(|list| list as &dyn MultivalueList));
            drop(budget);
            visitor(row, &views)?;
            ordinal += 1;
            Ok(())
        })?;
        if ordinal != parents {
            return Err(QueryError::Source);
        }
        Ok(())
    }
}
