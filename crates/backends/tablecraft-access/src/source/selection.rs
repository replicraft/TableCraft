use super::{NativeSource, query_error, schema::ScalarSchema};
use crate::collation::identity;
use tablecraft_ace::{
    BinaryIndexSelection, BooleanIndexSelection, ByteIndexSelection, CurrencyIndexSelection,
    GeneralEqualityPrefix, GuidIndexSelection, IndexInspection, IndexProjectionSelection,
    IndexReadRequest, Int16IndexSelection, Int32IndexSelection, Int64IndexSelection, NativeIndex,
    TextIndexSelection, inspect_table_indexes, read_table_index_in_table_order,
    read_table_index_with_request,
};
use tablecraft_core::{limits::ResourceBudget, values::Value};
use tablecraft_query::{QueryError, Source, source::ScanConstraint};
use tablecraft_schema::ScalarType;

mod automatic;

impl<R: tablecraft_ace::NativeReadSource> NativeSource<'_, R> {
    pub(super) fn selected_scan(
        &self,
        table: &str,
        constraint: ScanConstraint,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), QueryError>,
    ) -> Result<(), QueryError> {
        let entry = self.entry(table)?;
        if entry.index.is_none() && !self.auto_indexes {
            return self.scan(table, visitor);
        }
        self.schema_columns(table)?;
        let declared = entry.columns.get().ok_or(QueryError::Source)?;
        if declared.has_cached(None) {
            return self.scan(table, visitor);
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
            let Some((index, selection)) = chosen_index(
                &inspection,
                entry.index.as_ref().map(|index| index.as_str()),
                declared,
                constraint,
                &mut budget,
                self.cancelled,
            )?
            else {
                drop(budget);
                drop(source);
                return self.scan(table, visitor);
            };
            if selection == IndexProjectionSelection::All {
                drop(budget);
                drop(source);
                return self.scan(table, visitor);
            }
            let names = if !declared.unavailable.is_empty() {
                Some(
                    declared
                        .projection_names(&mut budget)
                        .map_err(query_error)?,
                )
            } else {
                None
            };
            let read_index = if entry.index.is_none() {
                read_table_index_in_table_order
            } else {
                read_table_index_with_request
            };
            read_index(
                &mut *source,
                self.scope,
                &entry.name,
                index,
                IndexReadRequest {
                    columns: names.as_deref(),
                    selection,
                    maximum_rows: self.snapshot_limits.maximum_rows(),
                },
                &mut budget,
                self.cancelled,
            )
            .map_err(query_error)?
        };
        self.check_snapshot_rows(&snapshot)?;
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
    Boolean(BooleanIndexSelection),
    Byte(ByteIndexSelection),
    Int16(Int16IndexSelection),
    Currency(CurrencyIndexSelection),
    Guid(GuidIndexSelection),
    Int32(Int32IndexSelection),
    Int64(Int64IndexSelection),
    Text(TextIndexSelection<'static>),
    Binary(BinaryIndexSelection<'static>),
}

fn selection(constraint: ScanConstraint, kind: ScalarType) -> Option<Selection> {
    match (constraint, kind) {
        (ScanConstraint::Boolean { value, .. }, ScalarType::Boolean) => {
            Some(Selection::Boolean(BooleanIndexSelection::Equal(value)))
        }
        (ScanConstraint::Null { .. }, ScalarType::Text) => {
            Some(Selection::Text(TextIndexSelection::Null))
        }
        (ScanConstraint::Null { .. }, ScalarType::Binary) => {
            Some(Selection::Binary(BinaryIndexSelection::Null))
        }
        (ScanConstraint::Null { .. }, ScalarType::Byte) => {
            Some(Selection::Byte(ByteIndexSelection::Null))
        }
        (ScanConstraint::Null { .. }, ScalarType::Int16) => {
            Some(Selection::Int16(Int16IndexSelection::Null))
        }
        (ScanConstraint::Byte { lower, upper, .. }, ScalarType::Byte) => {
            Some(Selection::Byte(ByteIndexSelection::Range { lower, upper }))
        }
        (ScanConstraint::Int16 { lower, upper, .. }, ScalarType::Int16) => {
            Some(Selection::Int16(Int16IndexSelection::Range {
                lower,
                upper,
            }))
        }
        (ScanConstraint::Null { .. }, ScalarType::Currency) => {
            Some(Selection::Currency(CurrencyIndexSelection::Null))
        }
        (ScanConstraint::Currency { lower, upper, .. }, ScalarType::Currency) => {
            Some(Selection::Currency(CurrencyIndexSelection::Range {
                lower,
                upper,
            }))
        }
        (ScanConstraint::Null { .. }, ScalarType::Guid) => {
            Some(Selection::Guid(GuidIndexSelection::Null))
        }
        (ScanConstraint::Guid { value, .. }, ScalarType::Guid) => {
            Some(Selection::Guid(GuidIndexSelection::Equal(value)))
        }
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
    inspection: &IndexInspection,
    index: &str,
    declared: &ScalarSchema,
    constraint: ScanConstraint,
) -> Result<IndexProjectionSelection<'static>, QueryError> {
    let selected = inspection
        .indexes()
        .iter()
        .find(|candidate| candidate.name().eq_ignore_ascii_case(index))
        .ok_or(QueryError::UnknownName)?;
    Ok(candidate_selection(
        inspection, selected, declared, constraint,
    ))
}

fn candidate_selection(
    inspection: &IndexInspection,
    selected: &NativeIndex,
    declared: &ScalarSchema,
    constraint: ScanConstraint,
) -> IndexProjectionSelection<'static> {
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
        return IndexProjectionSelection::All;
    }
    if let ScanConstraint::TextEqual { key: hint, .. } = constraint {
        let Some(field) = key else {
            return IndexProjectionSelection::All;
        };
        let Some(order) = field.text_sort_order() else {
            return IndexProjectionSelection::All;
        };
        if hint.policy() != identity(order) {
            return IndexProjectionSelection::All;
        }
        let Ok(prefix) = GeneralEqualityPrefix::from_encoded(order, hint.as_bytes()) else {
            return IndexProjectionSelection::All;
        };
        return if prefix.supports_column(field) {
            IndexProjectionSelection::Text(TextIndexSelection::EqualityPrefix(prefix))
        } else {
            IndexProjectionSelection::All
        };
    }

    match key.and_then(|field| selection(constraint, field.scalar_type()?)) {
        Some(Selection::Boolean(selection)) => IndexProjectionSelection::Boolean(selection),
        Some(Selection::Byte(selection)) => IndexProjectionSelection::Byte(selection),
        Some(Selection::Int16(selection)) => IndexProjectionSelection::Int16(selection),
        Some(Selection::Currency(selection)) => IndexProjectionSelection::Currency(selection),
        Some(Selection::Guid(selection)) => IndexProjectionSelection::Guid(selection),
        Some(Selection::Int32(selection)) => IndexProjectionSelection::Int32(selection),
        Some(Selection::Int64(selection)) => IndexProjectionSelection::Int64(selection),
        Some(Selection::Text(selection)) => IndexProjectionSelection::Text(selection),
        Some(Selection::Binary(selection)) => IndexProjectionSelection::Binary(selection),
        None => IndexProjectionSelection::All,
    }
}

pub(super) fn chosen_index<'a>(
    inspection: &'a IndexInspection,
    explicit: Option<&'a str>,
    declared: &ScalarSchema,
    constraint: ScanConstraint,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<(&'a str, IndexProjectionSelection<'static>)>, QueryError> {
    if let Some(index) = explicit {
        return projected_selection(inspection, index, declared, constraint)
            .map(|selection| Some((index, selection)));
    }
    automatic::choose(inspection, declared, constraint, budget, cancelled)
}

#[cfg(test)]
#[path = "selection/null_tests.rs"]
mod null_tests;
