use super::super::TableSchema;
use super::super::{record, snapshot::TableSnapshot};
use super::{Int32IndexSelection, Int64IndexSelection, inspect_table_indexes, walk::Walker};
use super::{
    NativeIndex, composite, guid,
    key::KeyProfile,
    page,
    profile::{IndexProfile, KeyType},
};
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
use tablecraft_schema::{Column, validate_row};

/// Read a completed snapshot through a named native one-to-ten-component
/// Boolean/Byte/Int16/Int32/Int64/Currency/Single/Double/GUID index.
/// Each component retains its qualified physical order.
/// Canonical GUID bytes and exact numeric carriers retain their qualified domains.
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
/// Other carriers and composite indexes are rejected, including All/Null.
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
        Some((KeyType::Int32, selection.widened())),
        budget,
        cancelled,
    )
}

/// Read a completed exact Int64/NULL selection through a qualified ACE 16-family
/// single-Int64 index. Bounds never pass through floating point. Other carriers
/// and composites are rejected even for All/Null; skipped subtrees are not checked.
/// Other source/profile/resource requirements match read_table_index_range.
pub fn read_table_int64_index_range<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    index: &str,
    selection: Int64IndexSelection,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSnapshot, InspectError> {
    read_selected(
        source,
        scope,
        table,
        index,
        Some((KeyType::Int64, selection)),
        budget,
        cancelled,
    )
}

fn read_selected<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    index: &str,
    selection: Option<(KeyType, Int64IndexSelection)>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSnapshot, InspectError> {
    let required_width = selection.map(|(width, _)| width);
    let selection = selection.map_or(Int64IndexSelection::All, |(_, selection)| selection);
    let complete = selection == Int64IndexSelection::All;
    let _span = tracing::debug_span!("native.index.read", bounded = !complete).entered();
    let inspection = inspect_table_indexes(source, scope, table, budget, cancelled)?;
    let selected = inspection
        .indexes
        .iter()
        .find(|x| x.name().eq_ignore_ascii_case(index))
        .ok_or(InspectError::UnknownIndex)?;
    let schema = &inspection.schema;
    let profile = IndexProfile::new(selected, schema)?;
    if schema.declared_rows() as usize > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit);
    }
    // A typed seek cannot become a complete composite scan, including All/Null.
    if required_width.is_some()
        && !matches!(&profile, IndexProfile::Single(single)
        if Some(single.key_type) == required_width)
    {
        return Err(InspectError::Unsupported);
    }
    let columns = super::super::snapshot::columns(schema, budget)?;
    let rows = match profile {
        IndexProfile::Single(mut profile) => {
            profile.selection = selection;
            read_rows::<_, _, { page::MAX_ENTRY_BYTES }>(
                source, schema, selected, &columns, profile, budget, cancelled,
            )?
        }
        IndexProfile::Composite(profile) => read_rows::<_, _, { composite::MAX_ENTRY_BYTES }>(
            source, schema, selected, &columns, profile, budget, cancelled,
        )?,
        IndexProfile::Guid(profile) => read_rows::<_, _, { guid::MAX_ENTRY_BYTES }>(
            source, schema, selected, &columns, profile, budget, cancelled,
        )?,
        IndexProfile::GuidComposite(profile) => {
            read_rows::<_, _, { composite::MAX_GUID_ENTRY_BYTES }>(
                source, schema, selected, &columns, profile, budget, cancelled,
            )?
        }
    };
    tracing::debug!(rows = rows.len(), "indexed table completed");
    Ok(TableSnapshot {
        schema: inspection.schema,
        columns,
        rows,
    })
}

fn read_rows<R: Read + Seek, P: KeyProfile, const N: usize>(
    source: &mut R,
    schema: &TableSchema,
    selected: &NativeIndex,
    columns: &[Column],
    profile: P,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Vec<Value>>, InspectError> {
    let complete = profile.complete();
    let mut reader = Reader {
        source,
        header: schema.header,
        budget,
        cancelled,
        reads: 0,
    };
    let owned = allocation::pages(&mut reader, selected.usage)?;
    let mut walker = Walker::<_, _, N> {
        reader: &mut reader,
        owner: schema.definition,
        owned: &owned,
        profile: &profile,
        visited: Vec::new(),
        entries: Vec::new(),
        previous_leaf: 0,
        next_leaf: 0,
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
        reader.work(profile.components())?;
        reader.budget.charge(Resource::Rows, 1)?;
        let page_number = reference_page(entry.row);
        let slot = reference_slot(entry.row);
        if data_pages.binary_search(&page_number).is_err() {
            return Err(InspectError::Malformed);
        }
        let raw = reader.live_row(page_number, slot, schema.definition)?;
        let values = record::decode(&raw, schema, &mut reader)?;
        validate_row(columns, &values)?;
        if profile.row_key(&values)? != entry.key {
            return Err(InspectError::Malformed);
        }
        reader
            .budget
            .charge_allocation(1, size_of::<Vec<Value>>() as u64)?;
        rows.try_reserve(1).map_err(|_| InspectError::Allocation)?;
        rows.push(values);
    }
    reader.tick()?;
    Ok(rows)
}
