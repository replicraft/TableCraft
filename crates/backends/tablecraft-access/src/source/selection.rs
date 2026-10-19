use super::{NativeSource, query_error};
use std::io::{Read, Seek};
use tablecraft_ace::{
    Int32IndexSelection, Int64IndexSelection, inspect_table_indexes, read_table_index_range,
    read_table_int64_index_range,
};
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
        self.schema_columns(table)?;
        if !entry
            .columns
            .get()
            .ok_or(QueryError::Source)?
            .unavailable
            .is_empty()
        {
            // Constraints index the dense scalar schema. Mixed-table native
            // indexed projections are unqualified; never reinterpret positions.
            return Err(QueryError::Unsupported);
        }
        if entry.table.get().is_some() {
            return self.scan(table, visitor);
        }
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
            let selection = key_column.and_then(|column| {
                selection(
                    constraint,
                    inspection.schema().columns()[column].scalar_type()?,
                )
            });
            if key_column != Some(constraint.column()) || selection.is_none() {
                // The explicit index remains mandatory. Release I/O borrows
                // before using its complete path; never switch to sequential.
                drop(budget);
                drop(source);
                return self.scan(table, visitor);
            }
            match selection.ok_or(QueryError::Unsupported)? {
                Selection::Int32(selection) => read_table_index_range(
                    &mut *source,
                    self.scope,
                    &entry.name,
                    index.as_str(),
                    selection,
                    &mut budget,
                    self.cancelled,
                )
                .map_err(query_error)?,
                Selection::Int64(selection) => read_table_int64_index_range(
                    &mut *source,
                    self.scope,
                    &entry.name,
                    index.as_str(),
                    selection,
                    &mut budget,
                    self.cancelled,
                )
                .map_err(query_error)?,
            }
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

enum Selection {
    Int32(Int32IndexSelection),
    Int64(Int64IndexSelection),
}
fn selection(constraint: ScanConstraint, kind: ScalarType) -> Option<Selection> {
    match (constraint, kind) {
        (ScanConstraint::Null { .. }, ScalarType::Int32) => {
            Some(Selection::Int32(Int32IndexSelection::Null))
        }
        (ScanConstraint::Null { .. }, ScalarType::Int64) => {
            Some(Selection::Int64(Int64IndexSelection::Null))
        }
        (ScanConstraint::Int32 { lower, upper, .. }, ScalarType::Int32) => {
            Some(Selection::Int32(Int32IndexSelection::Range {
                lower,
                upper,
            }))
        }
        (ScanConstraint::Int64 { lower, upper, .. }, ScalarType::Int64) => {
            Some(Selection::Int64(Int64IndexSelection::Range {
                lower,
                upper,
            }))
        }
        _ => None,
    }
}
