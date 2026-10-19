use super::{NativeSource, query_error};
use std::io::{Read, Seek};
use tablecraft_ace::{TableSnapshot, read_table, read_table_index, read_table_projection};
use tablecraft_query::QueryError;

impl<R: Read + Seek> NativeSource<'_, R> {
    pub(super) fn table(&self, name: &str) -> Result<&TableSnapshot, QueryError> {
        self.schema_columns(name)?;
        let entry = self.entry(name)?;
        let declared = entry.columns.get().ok_or(QueryError::Source)?;
        if entry.index.is_some() && !declared.unavailable.is_empty() {
            return Err(QueryError::Unsupported);
        }
        tracing::debug!(cached = entry.table.get().is_some(), "native table access");
        if entry.table.get().is_none() {
            let mut source = self
                .source
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            let mut budget = self
                .budget
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            let table = if let Some(index) = &entry.index {
                read_table_index(
                    &mut *source,
                    self.scope,
                    &entry.name,
                    index.as_str(),
                    &mut budget,
                    self.cancelled,
                )
            } else if !declared.unavailable.is_empty() {
                budget
                    .charge_allocation(declared.columns.len() as u64, size_of::<&str>() as u64)
                    .map_err(|error| query_error(error.into()))?;
                let mut names = Vec::new();
                names
                    .try_reserve_exact(declared.columns.len())
                    .map_err(|_| QueryError::Allocation)?;
                names.extend(declared.columns.iter().map(|column| column.name()));
                read_table_projection(
                    &mut *source,
                    self.scope,
                    &entry.name,
                    &names,
                    &mut budget,
                    self.cancelled,
                )
            } else {
                read_table(
                    &mut *source,
                    self.scope,
                    &entry.name,
                    &mut budget,
                    self.cancelled,
                )
            }
            .map_err(query_error)?;
            let current_unavailable = table
                .schema()
                .columns()
                .iter()
                .filter(|field| field.scalar_type().is_none());
            if declared.columns != table.columns()
                || !current_unavailable
                    .map(|field| field.name())
                    .eq(declared.unavailable.iter().map(|name| name.as_str()))
            {
                return Err(QueryError::SchemaChanged);
            }
            entry.table.set(table).map_err(|_| QueryError::Source)?;
        }
        entry.table.get().ok_or(QueryError::Source)
    }
}
