use super::{NativeSource, query_error};
use std::io::{Read, Seek};
use tablecraft_ace::{
    IndexProjection, IndexProjectionSelection, Int32IndexSelection, Int64IndexSelection,
    inspect_table_indexes, read_table_index_projection, read_table_index_range,
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
        let declared = entry.columns.get().ok_or(QueryError::Source)?;
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
            let selection = projected_selection(&inspection, index.as_str(), declared, constraint)?;
            if selection == IndexProjectionSelection::All {
                drop(budget);
                drop(source);
                return self.scan(table, visitor);
            }
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
                        selection,
                    },
                    &mut budget,
                    self.cancelled,
                )
                .map_err(query_error)?
            } else {
                match selection {
                    IndexProjectionSelection::Int32(selection) => read_table_index_range(
                        &mut *source,
                        self.scope,
                        &entry.name,
                        index.as_str(),
                        selection,
                        &mut budget,
                        self.cancelled,
                    )
                    .map_err(query_error)?,
                    IndexProjectionSelection::Int64(selection) => read_table_int64_index_range(
                        &mut *source,
                        self.scope,
                        &entry.name,
                        index.as_str(),
                        selection,
                        &mut budget,
                        self.cancelled,
                    )
                    .map_err(query_error)?,
                    IndexProjectionSelection::All => return Err(QueryError::Source),
                }
            }
        };
        if !declared.matches(&snapshot) {
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

pub(super) fn projected_selection(
    inspection: &tablecraft_ace::IndexInspection,
    index: &str,
    declared: &super::schema::ScalarSchema,
    constraint: ScanConstraint,
) -> Result<IndexProjectionSelection, QueryError> {
    let selected = inspection
        .indexes()
        .iter()
        .find(|candidate| candidate.name().eq_ignore_ascii_case(index))
        .ok_or(QueryError::UnknownName)?;
    let key = match selected.columns() {
        [key] => inspection
            .schema()
            .columns()
            .iter()
            .find(|field| field.number() == key.number()),
        _ => None,
    };
    let dense = key.and_then(|field| {
        declared
            .columns
            .iter()
            .position(|column| column.name().eq_ignore_ascii_case(field.name()))
    });
    if dense != Some(constraint.column()) {
        return Ok(IndexProjectionSelection::All);
    }
    Ok(
        match key.and_then(|field| selection(constraint, field.scalar_type()?)) {
            Some(Selection::Int32(selection)) => IndexProjectionSelection::Int32(selection),
            Some(Selection::Int64(selection)) => IndexProjectionSelection::Int64(selection),
            None => IndexProjectionSelection::All,
        },
    )
}
