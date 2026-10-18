use super::{NativeSource, query_error};
use std::io::{Read, Seek};
use tablecraft_ace::{Int32IndexSelection, inspect_table_indexes, read_table_index_range};
use tablecraft_core::values::Value;
use tablecraft_query::{QueryError, Source, source::ScanConstraint};
use tablecraft_schema::ScalarType;

impl<R: Read + Seek> NativeSource<'_, R> {
    pub(super) fn selected_scan(
        &self,
        table: &str,
        constraint: ScanConstraint,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        let entry = self.entry(table)?;
        let Some(index) = &entry.index else {
            return self.scan(table, visitor);
        };
        if entry.table.get().is_some() {
            return self.scan(table, visitor);
        }
        let selection = match constraint {
            ScanConstraint::Null { .. } => Int32IndexSelection::Null,
            ScanConstraint::Int32 { lower, upper, .. } => {
                Int32IndexSelection::Range { lower, upper }
            }
        };
        let snapshot = {
            let mut source = self
                .source
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            let mut budget = self
                .budget
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            let inspection = inspect_table_indexes(
                &mut *source,
                self.scope,
                &entry.name,
                &mut budget,
                self.cancelled,
            )
            .map_err(query_error)?;
            let selected = inspection
                .indexes()
                .iter()
                .find(|candidate| candidate.name().eq_ignore_ascii_case(index.as_str()))
                .ok_or(QueryError::UnknownName)?;
            let key_column = match selected.columns() {
                [key] => inspection
                    .schema()
                    .columns()
                    .iter()
                    .position(|column| column.number() == key.number()),
                _ => None,
            };
            let int32_key = key_column.is_some_and(|column| {
                inspection.schema().columns()[column].scalar_type() == Some(ScalarType::Int32)
            });
            if key_column != Some(constraint.column()) || !int32_key {
                // The explicit index remains mandatory. Release I/O borrows
                // before using its complete path; never switch to sequential.
                drop(budget);
                drop(source);
                return self.scan(table, visitor);
            }
            read_table_index_range(
                &mut *source,
                self.scope,
                &entry.name,
                index.as_str(),
                selection,
                &mut budget,
                self.cancelled,
            )
            .map_err(query_error)?
        };
        if self.schema_columns(table)? != snapshot.columns() {
            return Err(QueryError::SchemaChanged);
        }
        tracing::debug!(
            rows = snapshot.rows().len(),
            "native selected scan completed"
        );
        // Partial snapshots are ephemeral: a later unconstrained scan must load
        // a complete table rather than reusing this subset as the table cache.
        for row in snapshot.rows() {
            if (self.cancelled)() {
                return Err(QueryError::Cancelled);
            }
            visitor(row)?;
        }
        Ok(())
    }
}
