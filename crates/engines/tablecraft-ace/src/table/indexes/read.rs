use super::super::{record, snapshot::TableSnapshot};
use super::profile::KeyWidth;
use super::{Int32IndexSelection, inspect_table_indexes, walk::Walker};
use crate::{
    InspectError,
    format::{
        allocation,
        reader::Reader,
        row::{reference_page, reference_slot},
    },
    limits::MAX_LIVE_ROWS,
};
use std::io::{Read, Seek};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::validate_row;

/// Read a completed table snapshot through a named native single-Int32/Int64 index.
/// Traverses validated index pages and fetches referenced live rows. No automatic
/// optimizer selection, constraint enforcement or concurrent-file snapshot is implied.
pub fn read_table_index<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    index: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSnapshot, InspectError> {
    read_selected(source, scope, table, index, None, budget, cancelled)
}

/// Read a completed Int32/NULL selection through a qualified native index.
/// Prunes disjoint child spans and fetches only matching live rows. Validation
/// covers traversed pages/returned rows, not skipped subtrees or full row counts.
/// Int64 indexes are rejected by this typed Int32 API, including All/Null.
/// Other index restrictions match read_table_index; no fallback occurs.
pub fn read_table_index_range<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    index: &str,
    selection: Int32IndexSelection,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSnapshot, InspectError> {
    read_selected(
        source,
        scope,
        table,
        index,
        Some(selection),
        budget,
        cancelled,
    )
}

fn read_selected<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    index: &str,
    selection: Option<Int32IndexSelection>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSnapshot, InspectError> {
    let int32_only = selection.is_some();
    let selection = selection.unwrap_or(Int32IndexSelection::All);
    let complete = selection == Int32IndexSelection::All;
    let _span = tracing::debug_span!("native.index.read", bounded = !complete).entered();
    let inspection = inspect_table_indexes(source, scope, table, budget, cancelled)?;
    let selected = inspection
        .indexes
        .iter()
        .find(|x| x.name().eq_ignore_ascii_case(index))
        .ok_or(InspectError::UnknownIndex)?;
    let schema = &inspection.schema;
    let (column, width) = super::integer_column(selected, schema)?;
    if int32_only && width != KeyWidth::Int32 {
        return Err(InspectError::Unsupported);
    }
    if schema.declared_rows() as usize > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit);
    }
    let columns = super::super::snapshot::columns(schema, budget)?;
    let mut reader = Reader {
        source,
        header: schema.header,
        budget,
        cancelled,
        reads: 0,
    };
    let owned = allocation::pages(&mut reader, selected.usage)?;
    let mut walker = Walker {
        reader: &mut reader,
        owner: schema.definition,
        owned: &owned,
        descending: selected.columns[0].descending(),
        width,
        visited: Vec::new(),
        entries: Vec::new(),
        previous_leaf: 0,
        next_leaf: 0,
        selection,
        gap: false,
    };
    walker.visit(selected.root, 0, None, None)?;
    if (walker.previous_leaf != 0 && ((walker.next_leaf != 0) != walker.gap))
        || walker.entries.len() > schema.declared_rows() as usize
        || (complete && walker.entries.len() != schema.declared_rows() as usize)
    {
        return Err(InspectError::Malformed);
    }
    let entries = walker.entries;
    let data_pages = allocation::pages(&mut reader, schema.layout.usage)?;
    let mut rows = Vec::new();
    for entry in entries {
        reader.tick()?;
        reader.budget.charge(Resource::Rows, 1)?;
        let page_number = reference_page(entry.row);
        let slot = reference_slot(entry.row);
        if data_pages.binary_search(&page_number).is_err() {
            return Err(InspectError::Malformed);
        }
        let raw = reader.live_row(page_number, slot, schema.definition)?;
        let values = record::decode(&raw, schema, &mut reader)?;
        validate_row(&columns, &values)?;
        let matches = match (&values[column], entry.key) {
            (Value::Null, None) => true,
            (Value::Int32(a), Some(b)) => width == KeyWidth::Int32 && i64::from(*a) == b,
            (Value::Int64(a), Some(b)) => width == KeyWidth::Int64 && *a == b,
            _ => false,
        };
        if !matches {
            return Err(InspectError::Malformed);
        }
        reader
            .budget
            .charge_allocation(1, size_of::<Vec<Value>>() as u64)?;
        rows.try_reserve(1).map_err(|_| InspectError::Allocation)?;
        rows.push(values);
    }
    reader.tick()?;
    tracing::debug!(rows = rows.len(), "indexed table completed");
    Ok(TableSnapshot {
        schema: inspection.schema,
        columns,
        rows,
    })
}
