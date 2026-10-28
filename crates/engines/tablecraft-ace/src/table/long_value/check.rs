//! Complete logical-root visit with explicit column-LVAL storage qualification.
use super::{
    maps::{self, MapInspection, Storage},
    model::{LongValueCheck, LongValueCheckStatus, LongValueCounts, TableLongValueCheck},
    profile,
    trace::{self, Kind},
};
use crate::{
    FileFormat, InspectError, NativeReadSource,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, PageCursor, RepresentedMap},
        bytes::u32_at,
        data_type::{LONG_TEXT, OLE},
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{Reader, live_row_range, row_count},
        row::{MASK_BITS, reference_page},
    },
    limits::MAX_LIVE_ROWS,
    table::{long_text, record_layout, schema::inspect_table_schema},
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_model::CatalogScope;

fn entry(number: usize, status: LongValueCheckStatus) -> LongValueCheck {
    LongValueCheck {
        number,
        maps: None,
        status,
        counts: LongValueCounts::default(),
        owned_pages: 0,
        candidate_pages: 0,
        owned_rows: 0,
        distinct_chunks: 0,
        shared_references: 0,
        shared_owned_pages: 0,
    }
}

/// Check qualified ordinary ACE 16 column-LVAL maps and all current Memo/OLE
/// descriptors on one stable/offline local table. Unknown/absent/historical map
/// profiles remain visible; they are not required by ordinary value reads.
/// Owned orphan rows and shared references are counted without claiming corruption
/// or exclusive write authority. No values, active content or partial report are
/// published. Budgets and cancellation accumulate across the complete operation.
pub fn check_table_long_values<R: NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableLongValueCheck, InspectError> {
    check_with_initialization(source, scope, table, None, budget, cancelled)
}

#[cfg(feature = "ace16-write-preview")]
pub(in crate::table) fn check_table_long_values_for_initialization<R: NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    columns: &[usize],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableLongValueCheck, InspectError> {
    check_with_initialization(source, scope, table, Some(columns), budget, cancelled)
}

fn check_with_initialization<R: NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    table: &str,
    initializing: Option<&[usize]>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableLongValueCheck, InspectError> {
    let _span = tracing::debug_span!("native.table.long_value.check").entered();
    let schema = inspect_table_schema(source, scope, table, budget, cancelled)?;
    if schema.declared_rows() as usize > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit);
    }

    let mut reader = Reader {
        source,
        header: schema.header,
        budget,
        cancelled,
        reads: 0,
    };
    reader.tick()?;
    reader.work(schema.columns.len())?;
    let format = matches!(
        reader.header.format(),
        FileFormat::Ace2016 | FileFormat::Ace2019
    ) && reader.header.subversion() == 0
        && !reader.header.encoded_pages();
    let declarations = if format
        && schema
            .columns
            .iter()
            .any(|column| matches!(column.native_type(), LONG_TEXT | OLE))
    {
        if let Some(columns) = initializing {
            maps::declarations_for_initialization(&mut reader, &schema, columns)?
        } else {
            maps::declarations(&mut reader, &schema)?
        }
    } else {
        maps::Declarations {
            entries: Vec::new(),
            complete: true,
        }
    };
    let capacity = schema
        .columns
        .len()
        .checked_add(declarations.entries.len())
        .ok_or(InspectError::StructureLimit)?;
    reader
        .budget
        .charge_allocation(capacity as u64, size_of::<LongValueCheck>() as u64)?;
    reader
        .budget
        .charge_allocation(capacity as u64, size_of::<Option<Storage>>() as u64)?;
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    let mut storages = Vec::new();
    storages
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    let mut global = None;

    reader.work(schema.columns.len())?;
    for field in schema
        .columns
        .iter()
        .filter(|field| matches!(field.native_type(), LONG_TEXT | OLE))
    {
        reader.tick()?;
        reader.work(declarations.entries.len())?;
        let declaration = declarations
            .entries
            .iter()
            .find(|value| value.number == field.number());
        let mut result = entry(field.number(), LongValueCheckStatus::Absent);
        result.maps = declaration.map(|value| value.references);
        let mut storage = None;
        if !format {
            result.status = LongValueCheckStatus::UnsupportedFormat;
        } else if !declarations.complete {
            result.status = LongValueCheckStatus::UnknownDeclaration;
        } else if !profile::ordinary(field) {
            result.status = LongValueCheckStatus::UnsupportedColumn;
        } else if let Some(references) = result.maps {
            if references.owned == 0 || references.candidate == 0 {
                result.status = LongValueCheckStatus::Absent;
            } else {
                match maps::inspect(&mut reader, references)? {
                    MapInspection::Unknown => result.status = LongValueCheckStatus::UnknownMap,
                    MapInspection::Indirect => result.status = LongValueCheckStatus::IndirectMaps,
                    MapInspection::Qualified(owned, candidate) => {
                        if global.is_none() {
                            let value = RepresentedMap::read(
                                &mut reader,
                                GLOBAL_FREE_REFERENCE,
                                MapRole::GlobalFree,
                            )?;
                            value.require_allocated_storage(&mut reader, &value)?;
                            value.require_allocated(&mut reader, schema.definition)?;
                            global = Some(value);
                        }
                        let value = maps::capture(
                            &mut reader,
                            references,
                            &owned,
                            &candidate,
                            global.as_ref().ok_or(InspectError::Malformed)?,
                        )?;
                        result.status = LongValueCheckStatus::Checked;
                        result.owned_pages = value.pages.len();
                        result.candidate_pages = value.candidate_pages;
                        result.owned_rows = value.rows;
                        storage = Some(value);
                    }
                }
            }
        }
        columns.push(result);
        storages.push(storage);
    }
    for declaration in declarations.entries {
        reader.tick()?;
        reader.work(schema.columns.len())?;
        let field = schema
            .columns
            .iter()
            .find(|field| field.number() == declaration.number);
        if field.is_some_and(|field| matches!(field.native_type(), LONG_TEXT | OLE)) {
            continue;
        }

        let mut result = entry(
            declaration.number,
            if field.is_none() {
                LongValueCheckStatus::HistoricalDeclaration
            } else {
                LongValueCheckStatus::UnknownDeclaration
            },
        );
        result.maps = Some(declaration.references);
        columns.push(result);
        storages.push(None);
    }

    let mut cursor = PageCursor::new(&mut reader, schema.layout.usage)?;
    let mut rows = 0;
    while let Some(number) = cursor.next(&mut reader)? {
        if let Some(global) = &global {
            maps::allocated(&mut reader, global, number)?;
        }

        let page = reader.page(number)?;
        if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || u32_at(&page, OWNER_OFFSET)? != schema.definition
        {
            return Err(InspectError::Malformed);
        }
        for slot in 0..row_count(&page)? {
            reader.tick()?;
            if live_row_range(&page, slot)?.is_none() {
                continue;
            }
            if rows >= schema.declared_rows() as usize {
                return Err(InspectError::Malformed);
            }
            reader.budget.charge(Resource::Rows, 1)?;
            let physical = reader.row_location(
                number,
                u8::try_from(slot).map_err(|_| InspectError::Malformed)?,
                schema.definition,
                &page,
            )?;
            let layout = record_layout::parse_values(&physical.data, &schema, reader.budget)?;
            for (result, storage) in columns.iter_mut().zip(&mut storages) {
                let Some(storage) = storage else {
                    continue;
                };
                reader.tick()?;
                reader.work(schema.columns.len())?;
                let field = schema
                    .columns
                    .iter()
                    .find(|field| field.number() == result.number)
                    .ok_or(InspectError::Malformed)?;
                let present = field.number() < layout.count
                    && physical.data[layout.mask_start + field.number() / MASK_BITS]
                        & (1 << (field.number() % MASK_BITS))
                        != 0;
                let descriptor = if present {
                    let start = *layout
                        .offsets
                        .get(field.field.variable)
                        .ok_or(InspectError::Unsupported)?;
                    let end = *layout
                        .offsets
                        .get(field.field.variable + 1)
                        .ok_or(InspectError::Unsupported)?;
                    physical
                        .data
                        .get(start..end)
                        .ok_or(InspectError::Malformed)?
                } else {
                    &[]
                };
                let traced = trace::read(&mut reader, descriptor)?;
                if field.native_type() == LONG_TEXT
                    && let Some(value) = &traced.value
                {
                    long_text::decode(&mut reader, value)?;
                }
                match traced.kind {
                    Kind::Null => result.counts.nulls += 1,
                    Kind::Inline => result.counts.inline += 1,
                    Kind::Single => result.counts.single += 1,
                    Kind::Chained => result.counts.chained += 1,
                }
                let levels = storage.pages.len().checked_ilog2().unwrap_or(0) as usize + 1;
                for chunk in traced.chunks {
                    reader.work(levels)?;
                    let page = reference_page(chunk.reference);
                    if storage.pages.binary_search(&page).is_err()
                        || chunk.offset / PAGE_BYTES != page as usize
                        || chunk
                            .offset
                            .checked_add(chunk.bytes)
                            .is_none_or(|end| end > (page as usize + 1) * PAGE_BYTES)
                    {
                        return Err(InspectError::Malformed);
                    }
                    reader
                        .budget
                        .charge_allocation(1, size_of::<u32>() as u64)?;
                    storage
                        .reached
                        .try_reserve(1)
                        .map_err(|_| InspectError::Allocation)?;
                    storage.reached.push(chunk.reference);
                }
            }
            rows += 1;
        }
    }
    if rows != schema.declared_rows() as usize {
        return Err(InspectError::Malformed);
    }
    for (result, storage) in columns.iter_mut().zip(&mut storages) {
        let Some(storage) = storage else {
            continue;
        };
        let levels = storage.reached.len().checked_ilog2().unwrap_or(0) as usize + 1;
        reader.work(
            storage
                .reached
                .len()
                .checked_mul(levels)
                .ok_or(InspectError::StructureLimit)?,
        )?;
        storage.reached.sort_unstable();
        let total = storage.reached.len();
        storage.reached.dedup();
        result.distinct_chunks = storage.reached.len();
        result.shared_references = total - result.distinct_chunks;
        if result.distinct_chunks > result.owned_rows {
            return Err(InspectError::Malformed);
        }
    }
    for (position, storage) in storages.iter().enumerate() {
        let Some(storage) = storage else {
            continue;
        };
        for page in &storage.pages {
            reader.tick()?;
            let mut shared = false;
            for (other, candidate) in storages.iter().enumerate() {
                reader.tick()?;
                if other != position
                    && let Some(candidate) = candidate
                {
                    reader.work(candidate.pages.len().checked_ilog2().unwrap_or(0) as usize + 1)?;
                    shared |= candidate.pages.binary_search(page).is_ok();
                }
            }
            columns[position].shared_owned_pages += usize::from(shared);
        }
    }
    reader.tick()?;
    Ok(TableLongValueCheck {
        schema,
        rows,
        columns,
    })
}
