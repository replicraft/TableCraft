use super::super::{record, snapshot::TableSnapshot};
use super::{inspect_table_indexes, walk::Walker};
use crate::{
    InspectError,
    format::{
        allocation,
        reader::Reader,
        row::{reference_page, reference_slot},
    },
    profile::MAX_LIVE_ROWS,
};
use std::io::{Read, Seek};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::{ScalarType, validate_row};

const IGNORE_NULLS: u8 = 0x02;
const KNOWN_FLAGS: u8 = 0x80 | 0x01 | IGNORE_NULLS | 0x08;

/// Read a completed table snapshot through a named native single-Int32 index.
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
    let _span = tracing::debug_span!("native.index.read").entered();
    let inspection = inspect_table_indexes(source, scope, table, budget, cancelled)?;
    let selected = inspection
        .indexes
        .iter()
        .find(|x| x.name().eq_ignore_ascii_case(index))
        .ok_or(InspectError::UnknownIndex)?;
    let schema = &inspection.schema;
    if selected.columns.len() != 1
        || selected.kind > 1
        || selected.flags & IGNORE_NULLS != 0
        || selected.flags & !KNOWN_FLAGS != 0
    {
        return Err(InspectError::Unsupported);
    }
    let column = schema
        .columns
        .iter()
        .position(|c| c.number() == selected.columns[0].number())
        .ok_or(InspectError::Malformed)?;
    if schema.columns[column].scalar_type() != Some(ScalarType::Int32) {
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
        visited: Vec::new(),
        entries: Vec::new(),
        previous_leaf: 0,
        next_leaf: 0,
    };
    walker.visit(selected.root, 0, None, None)?;
    if walker.next_leaf != 0 || walker.entries.len() != schema.declared_rows() as usize {
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
            (Value::Int32(a), Some(b)) => *a == b,
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
