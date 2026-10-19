use super::{NativeSource, query_error};
use std::io::{Read, Seek};
use tablecraft_ace::inspect_table_schema;
use tablecraft_query::QueryError;
use tablecraft_schema::Column;

impl<R: Read + Seek> NativeSource<'_, R> {
    pub(super) fn schema_columns(&self, table: &str) -> Result<&[Column], QueryError> {
        let entry = self.entry(table)?;
        if let Some(snapshot) = entry.table.get() {
            return Ok(snapshot.columns());
        }
        tracing::debug!(
            cached = entry.columns.get().is_some(),
            "native schema access"
        );
        if entry.columns.get().is_none() {
            let mut source = self
                .source
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            let mut budget = self
                .budget
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            let schema = inspect_table_schema(
                &mut *source,
                self.scope,
                &entry.name,
                &mut budget,
                self.cancelled,
            )
            .map_err(query_error)?;
            let columns = schema
                .scalar_columns_with_budget(&mut budget)
                .map_err(query_error)?;
            entry.columns.set(columns).map_err(|_| QueryError::Source)?;
        }
        entry
            .columns
            .get()
            .map(Vec::as_slice)
            .ok_or(QueryError::Source)
    }
}
