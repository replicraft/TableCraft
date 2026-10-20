use super::super::TableSchema;
use super::super::{record, snapshot::TableSnapshot};
use super::binary::BinaryIndexSelection;
use super::key::KeyWork;
use super::projection::{IndexProjection, IndexProjectionSelection, RowProjection};
use super::{Int32IndexSelection, Int64IndexSelection, inspect_table_indexes, walk::Walker};
use super::{
    NativeIndex, composite, decimal, guid,
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
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::validate_row;

/// Read a completed snapshot through a named native one-to-ten-component
/// Boolean/Byte/Int16/Int32/Int64/Currency/Single/Double/DateTime/GUID index.
/// Each component retains its qualified physical order.
/// ACE 16-family Decimal components also retain exact precision/scale in
/// single or composite indexes, including mixed Decimal/GUID tuples.
/// Canonical GUID bytes and exact numeric carriers retain their qualified domains.
/// Single Short Binary and General Short Text indexes have separately qualified
/// physical key profiles; text keys include entry-level overflow digests.
/// Traverses validated index pages and fetches referenced live rows. No automatic
/// optimizer selection, constraint enforcement or concurrent-file snapshot is implied.
pub fn read_table_index<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    index: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSnapshot, InspectError> {
    read_selected(
        source,
        scope,
        table,
        index,
        ReadOptions {
            selection: None,
            columns: None,
        },
        budget,
        cancelled,
    )
}

/// Read a completed Int32/NULL selection through a qualified native index.
/// Prunes disjoint child spans and fetches only matching live rows. Validation
/// covers traversed pages/returned rows, not skipped subtrees or full row counts.
/// Other carriers and composite indexes are rejected, including All/Null.
/// Other index restrictions match read_table_index; no fallback occurs.
pub fn read_table_index_range<R: crate::NativeReadSource>(
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
        ReadOptions {
            selection: Some(ReadSelection::Integer(KeyType::Int32, selection.widened())),
            columns: None,
        },
        budget,
        cancelled,
    )
}

/// Read a completed exact Int64/NULL selection through a qualified ACE 16-family
/// single-Int64 index. Bounds never pass through floating point. Other carriers
/// and composites are rejected even for All/Null; skipped subtrees are not checked.
/// Other source/profile/resource requirements match read_table_index_range.
pub fn read_table_int64_index_range<R: crate::NativeReadSource>(
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
        ReadOptions {
            selection: Some(ReadSelection::Integer(KeyType::Int64, selection)),
            columns: None,
        },
        budget,
        cancelled,
    )
}

/// Read an explicit scalar projection through a mandatory qualified native index.
/// All physical key fields are decoded and compared with each returned index key,
/// including keys absent from output. Unselected non-key payloads remain unchecked.
/// Full traversal checks complete counts; bounded selections retain existing
/// traversed-page/returned-row scope. No omitted fields become NULL or fallback.
pub fn read_table_index_projection<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    index: &str,
    projection: IndexProjection<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSnapshot, InspectError> {
    let selection = match projection.selection {
        IndexProjectionSelection::All => None,
        IndexProjectionSelection::Int32(selection) => {
            Some(ReadSelection::Integer(KeyType::Int32, selection.widened()))
        }
        IndexProjectionSelection::Int64(selection) => {
            Some(ReadSelection::Integer(KeyType::Int64, selection))
        }
        IndexProjectionSelection::Binary(selection) => Some(ReadSelection::Binary(selection)),
    };
    read_selected(
        source,
        scope,
        table,
        index,
        ReadOptions {
            selection,
            columns: Some(projection.columns),
        },
        budget,
        cancelled,
    )
}

/// Read exact Short Binary/NULL bounds through a mandatory single-column index.
/// Bounds use ascending byte order in either direction. Disjoint physical spans
/// are conservatively pruned and only matching roots are fetched. Skipped pages
/// and payloads are not audited. No fallback or automatic optimizer choice occurs.
pub fn read_table_binary_index_range<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    index: &str,
    selection: BinaryIndexSelection<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSnapshot, InspectError> {
    read_selected(
        source,
        scope,
        table,
        index,
        ReadOptions {
            selection: Some(ReadSelection::Binary(selection)),
            columns: None,
        },
        budget,
        cancelled,
    )
}
#[derive(Clone, Copy)]
enum ReadSelection<'a> {
    Integer(KeyType, Int64IndexSelection),
    Binary(BinaryIndexSelection<'a>),
}
impl ReadSelection<'_> {
    fn complete(self) -> bool {
        match self {
            Self::Integer(_, selection) => selection == Int64IndexSelection::All,
            Self::Binary(selection) => selection == BinaryIndexSelection::All,
        }
    }
}
struct ReadOptions<'a> {
    selection: Option<ReadSelection<'a>>,
    columns: Option<&'a [&'a str]>,
}

fn read_selected<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    index: &str,
    options: ReadOptions<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSnapshot, InspectError> {
    let required_width = match options.selection {
        Some(ReadSelection::Integer(kind, _)) => Some(kind),
        _ => None,
    };
    let selection = match options.selection {
        Some(ReadSelection::Integer(_, selection)) => selection,
        _ => Int64IndexSelection::All,
    };
    let binary_selection = match options.selection {
        Some(ReadSelection::Binary(selection)) => {
            selection.validate()?;
            Some(selection)
        }
        _ => None,
    };
    let complete = options.selection.is_none_or(ReadSelection::complete);
    let _span = tracing::debug_span!("native.index.read", bounded = !complete).entered();
    let mut inspection = inspect_table_indexes(source, scope, table, budget, cancelled)?;
    let position = inspection
        .indexes
        .iter()
        .position(|x| x.name().eq_ignore_ascii_case(index))
        .ok_or(InspectError::UnknownIndex)?;
    if inspection.indexes[position].native_type() == super::FOREIGN_INDEX {
        super::links::qualify(source, scope, &mut inspection, budget, cancelled)?;
    }
    let selected = &inspection.indexes[position];
    let schema = &inspection.schema;
    let profile = IndexProfile::new(selected, schema)?;
    if binary_selection.is_some() && !matches!(&profile, IndexProfile::Binary(_)) {
        return Err(InspectError::Unsupported);
    }
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
    let projection = RowProjection::new(schema, selected, options.columns, budget, cancelled)?;
    let rows = match profile {
        IndexProfile::Text(profile) => read_rows::<_, _, { super::text::MAX_ENTRY_BYTES }>(
            source,
            schema,
            selected,
            &projection,
            profile,
            budget,
            cancelled,
        )?,
        IndexProfile::Binary(mut profile) => {
            profile.selection = binary_selection.unwrap_or(BinaryIndexSelection::All);
            read_rows::<_, _, { super::binary::MAX_ENTRY_BYTES }>(
                source,
                schema,
                selected,
                &projection,
                profile,
                budget,
                cancelled,
            )?
        }
        IndexProfile::Single(mut profile) => {
            profile.selection = selection;
            read_rows::<_, _, { page::MAX_ENTRY_BYTES }>(
                source,
                schema,
                selected,
                &projection,
                profile,
                budget,
                cancelled,
            )?
        }
        IndexProfile::Composite(profile) => read_rows::<_, _, { composite::MAX_ENTRY_BYTES }>(
            source,
            schema,
            selected,
            &projection,
            profile,
            budget,
            cancelled,
        )?,
        IndexProfile::Guid(profile) => read_rows::<_, _, { guid::MAX_ENTRY_BYTES }>(
            source,
            schema,
            selected,
            &projection,
            profile,
            budget,
            cancelled,
        )?,
        IndexProfile::Decimal(profile) => read_rows::<_, _, { decimal::MAX_ENTRY_BYTES }>(
            source,
            schema,
            selected,
            &projection,
            profile,
            budget,
            cancelled,
        )?,
        IndexProfile::GuidComposite(profile) => {
            read_rows::<_, _, { composite::MAX_GUID_ENTRY_BYTES }>(
                source,
                schema,
                selected,
                &projection,
                profile,
                budget,
                cancelled,
            )?
        }
        IndexProfile::DecimalComposite(profile) => {
            // Decimal components are smaller than the qualified GUID maximum;
            // mixed Decimal/GUID tuples retain that same strict buffer ceiling.
            read_rows::<_, _, { composite::MAX_GUID_ENTRY_BYTES }>(
                source,
                schema,
                selected,
                &projection,
                profile,
                budget,
                cancelled,
            )?
        }
    };
    tracing::debug!(rows = rows.len(), "indexed table completed");
    Ok(TableSnapshot {
        schema: inspection.schema,
        columns: projection.output_columns,
        rows,
    })
}

fn read_rows<R: crate::NativeReadSource, P: KeyProfile, const N: usize>(
    source: &mut R,
    schema: &TableSchema,
    selected: &NativeIndex,
    projection: &RowProjection,
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
    let mut current_page = None;
    for entry in entries {
        reader.work(profile.components())?;
        reader.budget.charge(Resource::Rows, 1)?;
        let page_number = reference_page(entry.row);
        let slot = reference_slot(entry.row);
        if data_pages.binary_search(&page_number).is_err() {
            return Err(InspectError::Malformed);
        }
        if !matches!(&current_page, Some((number, _)) if *number == page_number) {
            current_page = Some((page_number, reader.page(page_number)?));
        }
        let (_, page) = current_page.as_ref().ok_or(InspectError::Malformed)?;
        let raw = reader.row_from_page(page_number, slot, schema.definition, page)?;
        let mut values = match projection.positions() {
            Some(positions) => record::decode_selected(&raw, schema, Some(positions), &mut reader)?,
            None => record::decode(&raw, schema, &mut reader)?,
        };
        validate_row(projection.columns(), &values)?;
        if profile.row_key(
            &projection.key_view(&values),
            &mut KeyWork {
                budget: reader.budget,
                cancelled: reader.cancelled,
            },
        )? != entry.key
        {
            return Err(InspectError::Malformed);
        }
        if projection.positions().is_some() {
            reader.work(values.len())?;
        }
        projection.finish(&mut values);
        reader
            .budget
            .charge_allocation(1, size_of::<Vec<Value>>() as u64)?;
        rows.try_reserve(1).map_err(|_| InspectError::Allocation)?;
        rows.push(values);
    }
    reader.tick()?;
    Ok(rows)
}
