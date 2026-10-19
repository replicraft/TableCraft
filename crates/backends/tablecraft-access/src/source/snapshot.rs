use super::{NativeSource, query_error};
use std::io::{Read, Seek};
use tablecraft_ace::{
    IndexProjection, IndexProjectionSelection, TableSnapshot, read_table, read_table_index,
    read_table_index_projection, read_table_projection,
};
use tablecraft_query::QueryError;

impl<R: Read + Seek> NativeSource<'_, R> {
    pub(super) fn table(&self, name: &str) -> Result<&TableSnapshot, QueryError> {
        self.schema_columns(name)?;
        let entry = self.entry(name)?;
        let declared = entry.columns.get().ok_or(QueryError::Source)?;
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
                if !declared.unavailable.is_empty() {
                    let names = declared
                        .projection_names(&mut budget)
                        .map_err(query_error)?;
                    read_table_index_projection(
                        &mut *source,
                        self.scope,
                        &entry.name,
                        index.as_str(),
                        IndexProjection {
                            columns: &names,
                            selection: IndexProjectionSelection::All,
                        },
                        &mut budget,
                        self.cancelled,
                    )
                } else {
                    read_table_index(
                        &mut *source,
                        self.scope,
                        &entry.name,
                        index.as_str(),
                        &mut budget,
                        self.cancelled,
                    )
                }
            } else if !declared.unavailable.is_empty() {
                let names = declared
                    .projection_names(&mut budget)
                    .map_err(query_error)?;
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
            if !declared.matches(&table) {
                return Err(QueryError::SchemaChanged);
            }
            entry.table.set(table).map_err(|_| QueryError::Source)?;
        }
        entry.table.get().ok_or(QueryError::Source)
    }
}
