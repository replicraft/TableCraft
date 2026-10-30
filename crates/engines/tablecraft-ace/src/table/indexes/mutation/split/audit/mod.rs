//! Re-derive split, map and opaque-byte authority from the frozen source.
mod fitting;
pub(super) mod fixed_batch;
mod framing;
mod maps;
mod nonroot;
#[cfg(test)]
mod purpose_tests;
mod retained;
#[cfg(test)]
mod retained_tests;
mod rows;
mod slot;
mod source;

use super::super::{encoding, plan::keys, profile::SingleProfile, replacement::SlotKeyChanges};
use super::{Plan, rows::Changes};
use crate::{
    EditError, IndexCheck, InspectError, NativeImage, NativeIndex, TableCheck, TableSchema,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    inspect_header,
    limits::MAX_IMAGE_BYTES,
    table::{
        indexes::{
            header::{IndexHeader, LEVEL_OFFSET},
            mutation::{IndexKeyUpdate, NumericKey},
            page::ENTRY_START,
        },
        mutation::{
            CertifiedData, fitted::growing::joint::certified::CertifiedRows, map_authority,
            variable_update::map_audit::placement_certificate::CertifiedPlacement,
        },
    },
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_OFFSET: usize = std::mem::offset_of!(IndexHeader, free);
const OWNER_OFFSET: usize = std::mem::offset_of!(IndexHeader, owner);
const PREFIX_OFFSET: usize = std::mem::offset_of!(IndexHeader, prefix);
const HEADER_BYTES: usize = size_of::<IndexHeader>();
pub(super) const AUDIT_CHUNK_BYTES: usize = 256;

pub(super) fn prove(
    image: &NativeImage,
    checked: &TableCheck,
    updates: &[IndexKeyUpdate],
    plan: &Plan,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    prove_with(
        image,
        checked,
        Updates::Fixed(updates),
        plan,
        after,
        budget,
        cancelled,
    )
}

pub(super) fn prove_slot(
    image: &NativeImage,
    checked: &TableCheck,
    changes: &SlotKeyChanges<'_>,
    plan: &Plan,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    prove_with(
        image,
        checked,
        Updates::Slot(changes),
        plan,
        after,
        budget,
        cancelled,
    )
}

pub(super) fn prove_slot_after_data(
    image: &NativeImage,
    data: &CertifiedData<'_>,
    plan: &Plan,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    data.phase().require_source(image)?;
    data.phase().require_after(after)?;
    let keys = data.keys(budget, cancelled)?;
    let changes = SlotKeyChanges {
        column: data.column(),
        updates: &keys,
    };
    prove_with(
        image,
        data.original(),
        Updates::AfterData(&changes, data),
        plan,
        after,
        budget,
        cancelled,
    )
}

pub(super) fn prove_rows_after_data(
    image: &NativeImage,
    data: &CertifiedRows<'_>,
    plan: &Plan,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    data.phase().require_source(image)?;
    data.phase().require_after(after)?;
    let keys = data.keys(budget, cancelled)?;
    let changes = Changes {
        columns: data.columns(),
        updates: &keys,
    };
    prove_with(
        image,
        data.original(),
        Updates::RowsAfterData(&changes, data),
        plan,
        after,
        budget,
        cancelled,
    )
}

#[derive(Clone, Copy)]
enum Updates<'a> {
    Fixed(&'a [IndexKeyUpdate]),
    Slot(&'a SlotKeyChanges<'a>),
    AfterData(&'a SlotKeyChanges<'a>, &'a CertifiedData<'a>),
    RowsAfterData(&'a Changes<'a>, &'a CertifiedRows<'a>),
}

impl<'a> Updates<'a> {
    fn data(self) -> Option<&'a CertifiedPlacement<'a>> {
        match self {
            Self::AfterData(_, data) => Some(data.phase()),
            Self::RowsAfterData(_, data) => Some(data.phase()),
            _ => None,
        }
    }

    fn slots(self) -> Option<&'a SlotKeyChanges<'a>> {
        match self {
            Self::Slot(changes) | Self::AfterData(changes, _) => Some(changes),
            Self::Fixed(_) | Self::RowsAfterData(_, _) => None,
        }
    }

    fn len(self) -> usize {
        match self {
            Self::Fixed(updates) => updates.len(),
            Self::RowsAfterData(changes, _) => changes.updates.len(),
            Self::Slot(changes) | Self::AfterData(changes, _) => changes.updates.len(),
        }
    }

    fn affects(self, schema: &TableSchema, index: &NativeIndex) -> bool {
        match self {
            Self::RowsAfterData(changes, _) => {
                !changes.updates.is_empty()
                    && changes.columns.iter().any(|position| {
                        schema.columns.get(*position).is_some_and(|column| {
                            index
                                .columns()
                                .iter()
                                .any(|key| key.number() == column.number())
                        })
                    })
            }
            Self::Fixed(updates) => updates.iter().any(|update| {
                schema.columns.get(update.column).is_some_and(|column| {
                    index
                        .columns()
                        .iter()
                        .any(|key| key.number() == column.number())
                })
            }),
            Self::Slot(changes) | Self::AfterData(changes, _) => {
                !changes.updates.is_empty()
                    && schema.columns.get(changes.column).is_some_and(|column| {
                        index
                            .columns()
                            .iter()
                            .any(|key| key.number() == column.number())
                    })
            }
        }
    }
}

fn prove_with(
    image: &NativeImage,
    checked: &TableCheck,
    updates: Updates<'_>,
    plan: &Plan,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if plan.append.original_length != image.as_bytes().len()
        || plan.append.length != after.len()
        || plan.append.length < plan.append.original_length
        || plan.append.length as u64 > MAX_IMAGE_BYTES
        || !plan.append.length.is_multiple_of(PAGE_BYTES)
    {
        return Err(EditError::Preservation);
    }
    let schema = checked.schema();
    let indexes = checked.indexes();
    if matches!(updates, Updates::RowsAfterData(_, _)) {
        let mut source = image.reader();
        let mut reader = Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let original = source::capture(&[checked], &mut reader)?;
        let census = map_authority::capture_catalog(
            image,
            reader.budget,
            cancelled,
            |schema, indexes, declarations, reader| {
                original.require_catalog_claims(schema, indexes, declarations, reader)
            },
        )?;
        census.require_exclusive(&original.owners, reader.budget, cancelled)?;
    }

    let mut source = image.reader();
    if plan.append.length != plan.append.original_length {
        allocated_source(
            &mut Reader {
                source: &mut source,
                header: image.catalog().header(),
                budget,
                cancelled,
                reads: 0,
            },
            schema,
            indexes,
        )?;
    }
    let mut actual_source = Cursor::new(after);
    let header = inspect_header(&mut actual_source, budget, cancelled)?;
    let mut actual = Reader {
        source: &mut actual_source,
        header,
        budget,
        cancelled,
        reads: 0,
    };
    let mut expected_edits = if let Some(data) = updates.data() {
        maps::seed(
            image,
            data,
            &mut Reader {
                source: &mut source,
                header: image.catalog().header(),
                budget: actual.budget,
                cancelled,
                reads: 0,
            },
        )?
    } else {
        maps::Expected::default()
    };
    let index_begin = updates.data().map_or(
        image.catalog().header().page_count(),
        CertifiedPlacement::end,
    );
    let mut next_eof = index_begin;
    let mut split_position = 0;
    let mut affected_pages = Vec::new();

    for (position, checked) in indexes.iter().enumerate() {
        check(actual.budget, cancelled)?;
        actual.work(position)?;
        let index = checked.index();
        if indexes[..position]
            .iter()
            .any(|prior| prior.index().data_number() == index.data_number())
        {
            continue;
        }
        let source_pages = allocation::pages(
            &mut Reader {
                source: &mut source,
                header: image.catalog().header(),
                budget: actual.budget,
                cancelled,
                reads: 0,
            },
            index.usage_map(),
        )?;
        let affected = updates.affects(schema, index);
        actual.work(updates.len().saturating_mul(index.columns().len()))?;
        let mut expected_pages = source_pages;
        if affected {
            actual
                .budget
                .charge_allocation(expected_pages.len() as u64, size_of::<u32>() as u64)
                .map_err(InspectError::from)?;
            affected_pages
                .try_reserve(expected_pages.len())
                .map_err(|_| InspectError::Allocation)?;
            affected_pages.extend_from_slice(&expected_pages);
        }
        let single_integral = index.columns().len() == 1
            && schema
                .columns
                .iter()
                .find(|column| column.number() == index.columns()[0].number())
                .and_then(|column| column.scalar_type())
                .is_some_and(super::supported_scalar);
        let mut split = false;
        let mut verified_nonroot = false;
        if affected
            && single_integral
            && let Updates::Fixed(updates) = updates
        {
            let column = schema
                .columns
                .iter()
                .position(|column| column.number() == index.columns()[0].number())
                .ok_or(InspectError::Malformed)?;
            let profile = SingleProfile {
                column,
                kind: NumericKey::new(
                    schema.columns[column]
                        .scalar_type()
                        .ok_or(InspectError::Unsupported)?,
                )?,
                descending: index.columns()[0].descending(),
            };
            let prepared = keys::prepare::<_, { encoding::MAX_ENTRY_BYTES }>(
                image,
                &mut Reader {
                    source: &mut source,
                    header: image.catalog().header(),
                    budget: actual.budget,
                    cancelled,
                    reads: 0,
                },
                index,
                schema.definition,
                updates,
                &profile,
            )?;
            let disposition = retained::prove::<_, { encoding::MAX_ENTRY_BYTES }>(
                &retained::Operation {
                    image,
                    index,
                    owner: schema.definition,
                    column,
                    profile: &profile,
                    tree: &prepared.tree,
                    keys: &prepared.entries,
                },
                plan,
                &mut actual,
                &mut retained::Progress {
                    next_eof: &mut next_eof,
                    map_edits: &mut expected_edits,
                    pages: &mut expected_pages,
                    split_position: &mut split_position,
                },
            )?;
            split = disposition.split_root;
            verified_nonroot = disposition.recursive;
        }
        if affected && let Some(changes) = updates.slots() {
            let disposition = slot::prove(
                &slot::Operation {
                    image,
                    schema,
                    index,
                    changes,
                },
                plan,
                &mut actual,
                &mut retained::Progress {
                    next_eof: &mut next_eof,
                    map_edits: &mut expected_edits,
                    pages: &mut expected_pages,
                    split_position: &mut split_position,
                },
            )?;
            split = disposition.split_root;
            verified_nonroot = disposition.recursive;
        }
        if affected && let Updates::RowsAfterData(changes, _) = updates {
            let disposition = rows::prove(
                &rows::Operation {
                    image,
                    schema,
                    index,
                    changes,
                },
                plan,
                &mut actual,
                &mut retained::Progress {
                    next_eof: &mut next_eof,
                    map_edits: &mut expected_edits,
                    pages: &mut expected_pages,
                    split_position: &mut split_position,
                },
            )?;
            split = disposition.split_root;
            verified_nonroot = disposition.recursive;
        }
        if affected
            && index.columns().len() > 1
            && let Updates::Fixed(updates) = updates
        {
            fitting::prove(
                &fitting::Operation {
                    image,
                    schema,
                    index,
                    updates,
                },
                &mut actual,
            )?;
        }
        actual.work(
            expected_pages
                .len()
                .saturating_mul(expected_pages.len().checked_ilog2().unwrap_or(0) as usize + 1),
        )?;
        expected_pages.sort_unstable();
        if allocation::pages(&mut actual, index.usage_map())? != expected_pages {
            return Err(EditError::Preservation);
        }
        for number in expected_pages
            .into_iter()
            .filter(|number| *number < image.catalog().header().page_count())
        {
            actual.tick()?;
            if verified_nonroot || split && number == index.root_page() {
                continue;
            }
            let start = number as usize * PAGE_BYTES;
            let old = image
                .as_bytes()
                .get(start..start + PAGE_BYTES)
                .ok_or(InspectError::Malformed)?;
            let new = after
                .get(start..start + PAGE_BYTES)
                .ok_or(EditError::Preservation)?;
            if !affected {
                unchanged(old, new, actual.budget, cancelled)?;
            } else {
                unchanged(
                    &old[..FREE_OFFSET],
                    &new[..FREE_OFFSET],
                    actual.budget,
                    cancelled,
                )?;
                unchanged(
                    &old[OWNER_OFFSET..PREFIX_OFFSET],
                    &new[OWNER_OFFSET..PREFIX_OFFSET],
                    actual.budget,
                    cancelled,
                )?;
                unchanged(
                    &old[LEVEL_OFFSET..HEADER_BYTES],
                    &new[LEVEL_OFFSET..HEADER_BYTES],
                    actual.budget,
                    cancelled,
                )?;
                let end = PAGE_BYTES
                    .checked_sub(usize::from(IndexHeader::read(new)?.free.get()))
                    .filter(|end| *end >= ENTRY_START)
                    .ok_or(EditError::Preservation)?;
                unchanged(&old[end..], &new[end..], actual.budget, cancelled)?;
            }
        }
    }
    if split_position != plan.certificates.len()
        || plan.added.len()
            != next_eof
                .checked_sub(index_begin)
                .ok_or(EditError::Preservation)? as usize
        || next_eof as usize * PAGE_BYTES != plan.append.length
    {
        return Err(EditError::Preservation);
    }
    for (position, page) in plan.added.iter().enumerate() {
        check(actual.budget, cancelled)?;
        if page.number as usize != index_begin as usize + position
            || page.bytes.len() != PAGE_BYTES
            || after.get(page.number as usize * PAGE_BYTES..(page.number as usize + 1) * PAGE_BYTES)
                != Some(page.bytes.as_slice())
        {
            return Err(EditError::Preservation);
        }
    }
    for patch in &plan.patches {
        actual.tick()?;
        actual.work(affected_pages.len())?;
        let number =
            u32::try_from(patch.offset / PAGE_BYTES).map_err(|_| EditError::Preservation)?;
        if !affected_pages.contains(&number)
            || patch.before.len() != patch.after.len()
            || patch
                .offset
                .checked_add(patch.after.len())
                .is_none_or(|end| end > (number as usize + 1) * PAGE_BYTES)
        {
            return Err(EditError::Preservation);
        }
    }
    maps::authority(
        image,
        schema.definition,
        indexes,
        plan,
        &expected_edits,
        &mut Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget: actual.budget,
            cancelled,
            reads: 0,
        },
    )?;
    maps::finish(
        &mut Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget: actual.budget,
            cancelled,
            reads: 0,
        },
        &expected_edits,
        plan,
        after,
    )?;
    if let Some(data) = updates.data() {
        maps::cover_suffix(
            &mut Reader {
                source: &mut source,
                header: image.catalog().header(),
                budget: actual.budget,
                cancelled,
                reads: 0,
            },
            &expected_edits,
            data,
            next_eof,
        )?;
    }
    Ok(())
}

fn unchanged(
    before: &[u8],
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if before.len() != after.len() {
        return Err(EditError::Preservation);
    }
    budget
        .charge(Resource::Steps, before.len() as u64)
        .map_err(InspectError::from)?;
    for (old, new) in before
        .chunks(AUDIT_CHUNK_BYTES)
        .zip(after.chunks(AUDIT_CHUNK_BYTES))
    {
        check(budget, cancelled)?;
        if old != new {
            return Err(EditError::Preservation);
        }
    }
    Ok(())
}

fn allocated_source(
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    schema: &TableSchema,
    indexes: &[IndexCheck],
) -> Result<(), EditError> {
    let global = RepresentedMap::read(reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    global.require_allocated_storage(reader, &global)?;
    global.require_allocated(reader, schema.definition)?;
    let data = RepresentedMap::read(reader, schema.layout.usage, MapRole::Owned)?;
    data.require_allocated_storage(reader, &global)?;
    for number in allocation::pages(reader, schema.layout.usage)? {
        global.require_allocated(reader, number)?;
    }
    for (position, index) in indexes.iter().enumerate() {
        reader.work(position)?;
        if indexes[..position]
            .iter()
            .any(|prior| prior.index().data_number() == index.index().data_number())
        {
            continue;
        }
        let owned = RepresentedMap::read(reader, index.index().usage_map(), MapRole::Owned)?;
        owned.require_allocated_storage(reader, &global)?;
        for number in allocation::pages(reader, index.index().usage_map())? {
            global.require_allocated(reader, number)?;
        }
    }
    Ok(())
}
