use super::{NativeSource, query_error};
use tablecraft_ace::{
    IndexProjection, IndexProjectionSelection, TableCursor, TableSnapshot, read_table_index,
    read_table_index_projection,
};
use tablecraft_query::QueryError;

impl<R: tablecraft_ace::NativeReadSource> NativeSource<'_, R> {
    pub(super) fn check_snapshot_rows(&self, snapshot: &TableSnapshot) -> Result<(), QueryError> {
        if snapshot.rows().len() > self.snapshot_limits.maximum_rows() as usize {
            return Err(QueryError::StructureLimit);
        }
        Ok(())
    }
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
                TableCursor::open_projection(
                    &mut *source,
                    self.scope,
                    &entry.name,
                    &names,
                    &mut budget,
                    self.cancelled,
                )
                .and_then(|cursor| {
                    cursor.into_snapshot(self.snapshot_limits, &mut budget, self.cancelled)
                })
            } else {
                TableCursor::open(
                    &mut *source,
                    self.scope,
                    &entry.name,
                    &mut budget,
                    self.cancelled,
                )
                .and_then(|cursor| {
                    cursor.into_snapshot(self.snapshot_limits, &mut budget, self.cancelled)
                })
            }
            .map_err(query_error)?;
            self.check_snapshot_rows(&table)?;
            if !declared.matches(&table) {
                return Err(QueryError::SchemaChanged);
            }
            entry.table.set(table).map_err(|_| QueryError::Source)?;
        }
        entry.table.get().ok_or(QueryError::Source)
    }
}
