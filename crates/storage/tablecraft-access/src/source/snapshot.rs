use super::{NativeSource, query_error};
use std::io::{Read, Seek};
use tablecraft_ace::{TableSnapshot, read_table, read_table_index};
use tablecraft_query::QueryError;

impl<R: Read + Seek> NativeSource<'_, R> {
    pub(super) fn table(&self, name: &str) -> Result<&TableSnapshot, QueryError> {
        let entry = self.entry(name)?;
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
            if entry
                .columns
                .get()
                .is_some_and(|columns| columns != table.columns())
            {
                return Err(QueryError::SchemaChanged);
            }
            entry.table.set(table).map_err(|_| QueryError::Source)?;
        }
        entry.table.get().ok_or(QueryError::Source)
    }
}
