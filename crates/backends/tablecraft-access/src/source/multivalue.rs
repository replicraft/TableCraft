//! Complete native collection caches, separate from scalar/index projections.
use super::{NativeSource, query_error};
use std::cell::OnceCell;
use tablecraft_ace::{
    MultivalueColumn, inspect_multivalue_declarations, read_multivalue_column_with_limits,
};
use tablecraft_query::{
    QueryError,
    source::{TextCollationId, validate_projection},
};
use tablecraft_schema::Column;

pub(super) struct CollectionCache {
    columns: Vec<Column>,
    text_collations: Vec<Option<TextCollationId>>,
    snapshots: Vec<OnceCell<MultivalueColumn>>,
}
impl<R: tablecraft_ace::NativeReadSource> NativeSource<'_, R> {
    pub(super) fn collection_columns(&self, table: &str) -> Result<&[Column], QueryError> {
        if !self.multivalues {
            return Ok(&[]);
        }
        if self.text_policy_scope.required_order().is_some() {
            return Err(QueryError::Unsupported);
        }
        self.schema_columns(table)?;
        let entry = self.entry(table)?;
        if entry.multivalues.get().is_none() {
            let mut budget = self
                .budget
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            let declarations = if entry
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
                inspect_multivalue_declarations(
                    &mut *source,
                    self.scope,
                    &entry.name,
                    &mut budget,
                    self.cancelled,
                )
                .map_err(query_error)?
            };
            budget
                .charge_allocation(
                    declarations.len() as u64,
                    (size_of::<Column>() + size_of::<Option<TextCollationId>>()) as u64,
                )
                .map_err(|error| query_error(error.into()))?;
            let mut columns = Vec::new();
            let mut text_collations = Vec::new();
            columns
                .try_reserve_exact(declarations.len())
                .map_err(|_| QueryError::Allocation)?;
            text_collations
                .try_reserve_exact(declarations.len())
                .map_err(|_| QueryError::Allocation)?;
            for declaration in declarations {
                if (self.cancelled)() {
                    return Err(QueryError::Cancelled);
                }
                budget.charge(tablecraft_core::limits::Resource::Steps, 1)?;
                text_collations.push(
                    declaration
                        .text_sort_order()
                        .map(crate::collation::identity),
                );
                columns.push(declaration.into_column());
            }
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
                .set(CollectionCache {
                    columns,
                    text_collations,
                    snapshots,
                })
                .map_err(|_| QueryError::Source)?;
        }
        Ok(&entry.multivalues.get().ok_or(QueryError::Source)?.columns)
    }
    pub(super) fn collection_text_collations(
        &self,
        table: &str,
    ) -> Result<&[Option<TextCollationId>], QueryError> {
        if !self.multivalues {
            return Ok(&[]);
        }
        self.collection_columns(table)?;
        Ok(&self
            .entry(table)?
            .multivalues
            .get()
            .ok_or(QueryError::Source)?
            .text_collations)
    }
    pub(super) fn load_values(
        &self,
        table: &str,
        collections: &[usize],
    ) -> Result<Vec<&MultivalueColumn>, QueryError> {
        let _span = tracing::debug_span!("native.multivalue.scan").entered();
        if collections.is_empty() {
            return Ok(Vec::new());
        }
        if !self.multivalues {
            return Err(QueryError::Unsupported);
        }
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
                let snapshot = read_multivalue_column_with_limits(
                    &mut *source,
                    self.scope,
                    &entry.name,
                    values[*index].name(),
                    self.snapshot_limits,
                    &mut budget,
                    self.cancelled,
                )
                .map_err(query_error)?;
                if snapshot.value_column().type_descriptor() != values[*index].type_descriptor()
                    || snapshot.value_column().nullable() != values[*index].nullable()
                    || snapshot
                        .value_text_sort_order()
                        .map(crate::collation::identity)
                        != cache.text_collations[*index]
                {
                    return Err(QueryError::SchemaChanged);
                }
                cell.set(snapshot).map_err(|_| QueryError::Source)?;
            }
        }
        self.budget
            .try_borrow_mut()
            .map_err(|_| QueryError::Source)?
            .charge_allocation(
                collections.len() as u64,
                size_of::<&MultivalueColumn>() as u64,
            )
            .map_err(|error| query_error(error.into()))?;
        let mut selected = Vec::new();
        selected
            .try_reserve_exact(collections.len())
            .map_err(|_| QueryError::Allocation)?;
        for index in collections {
            selected.push(cache.snapshots[*index].get().ok_or(QueryError::Source)?);
        }
        Ok(selected)
    }
}
