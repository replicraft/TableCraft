//! Re-derive split, map and opaque-byte authority from the frozen source.
mod framing;
mod maps;

use super::super::{encoding, plan::keys, profile::SingleProfile, repack};
use super::{Plan, leaf_entries};
use crate::{
    EditError, IndexCheck, InspectError, NativeImage, TableCheck, TableSchema,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        page::PAGE_BYTES,
        reader::Reader,
    },
    header::check,
    inspect_header,
    limits::MAX_IMAGE_BYTES,
    table::indexes::{
        header::{IndexHeader, LEVEL_OFFSET},
        mutation::{IndexKeyUpdate, NumericKey},
        page::{self, ENTRY_START},
    },
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

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
    if plan.original_length != image.as_bytes().len()
        || plan.length != after.len()
        || plan.length < plan.original_length
        || plan.length as u64 > MAX_IMAGE_BYTES
        || !plan.length.is_multiple_of(PAGE_BYTES)
    {
        return Err(EditError::Preservation);
    }
    let schema = checked.schema();
    let indexes = checked.indexes();
    let mut source = image.reader();
    if !plan.certificates.is_empty() {
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
    let mut expected_edits = Vec::new();
    let mut next_eof = image.catalog().header().page_count();
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
        let affected = updates.iter().any(|update| {
            schema.columns.get(update.column).is_some_and(|column| {
                index
                    .columns()
                    .iter()
                    .any(|key| key.number() == column.number())
            })
        });
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
        let single_i32 = index.columns().len() == 1
            && schema
                .columns
                .iter()
                .find(|column| column.number() == index.columns()[0].number())
                .is_some_and(|column| column.scalar_type() == Some(ScalarType::Int32));
        let mut split = false;
        if affected && single_i32 {
            let column = schema
                .columns
                .iter()
                .position(|column| column.number() == index.columns()[0].number())
                .ok_or(InspectError::Malformed)?;
            let profile = SingleProfile {
                column,
                kind: NumericKey::new(ScalarType::Int32)?,
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
            let original_root = image
                .as_bytes()
                .get(
                    index.root_page() as usize * PAGE_BYTES
                        ..(index.root_page() as usize + 1) * PAGE_BYTES,
                )
                .ok_or(InspectError::Malformed)?;
            if prepared.tree.is_leaf() {
                let entries = leaf_entries(&prepared.entries, actual.budget, cancelled)?;
                split = !repack::fits::<_, { encoding::MAX_ENTRY_BYTES }>(
                    original_root,
                    &entries,
                    &profile,
                    actual.budget,
                    cancelled,
                )?;
            }
            if split {
                let right = next_eof;
                let left = right.checked_add(1).ok_or(EditError::Preservation)?;
                let fact = plan
                    .certificates
                    .get(split_position)
                    .ok_or(EditError::Preservation)?;
                if fact.physical != index.data_number()
                    || fact.root != index.root_page()
                    || fact.usage != index.usage_map()
                    || fact.column != column
                    || fact.descending != profile.descending
                    || fact.right != right
                    || fact.left != left
                    || fact.count != prepared.entries.len()
                    || fact.count < 2
                    || expected_pages != [fact.root]
                {
                    return Err(EditError::Preservation);
                }
                let middle = prepared.entries.len() / 2;
                let root = page::read_profile::<_, _, { encoding::MAX_ENTRY_BYTES }>(
                    &mut actual,
                    fact.root,
                    schema.definition,
                    &profile,
                )?;
                let l = page::read_profile::<_, _, { encoding::MAX_ENTRY_BYTES }>(
                    &mut actual,
                    left,
                    schema.definition,
                    &profile,
                )?;
                let r = page::read_profile::<_, _, { encoding::MAX_ENTRY_BYTES }>(
                    &mut actual,
                    right,
                    schema.definition,
                    &profile,
                )?;
                if root.leaf
                    || root.entries.len() != 1
                    || root.entries[0].child != left
                    || root.entries[0].value != prepared.entries[middle - 1]
                    || root.tail != right
                    || !l.leaf
                    || !r.leaf
                    || l.entries.len() != middle
                    || r.entries.len() != prepared.entries.len() - middle
                {
                    return Err(EditError::Preservation);
                }
                for (entry, expected) in l.entries.iter().chain(&r.entries).zip(&prepared.entries) {
                    actual.tick()?;
                    if entry.child != 0 || entry.value != *expected {
                        return Err(EditError::Preservation);
                    }
                }
                let root_raw = actual.page(fact.root)?;
                framing::root(
                    original_root,
                    &root_raw,
                    fact,
                    prepared.entries[middle - 1],
                    &profile,
                    actual.budget,
                    cancelled,
                )?;
                let left_raw = actual.page(left)?;
                framing::leaf(
                    &left_raw,
                    schema.definition,
                    (0, right),
                    &prepared.entries[..middle],
                    &profile,
                    actual.budget,
                    cancelled,
                )?;
                let right_raw = actual.page(right)?;
                framing::leaf(
                    &right_raw,
                    schema.definition,
                    (left, 0),
                    &prepared.entries[middle..],
                    &profile,
                    actual.budget,
                    cancelled,
                )?;
                maps::add(
                    &mut Reader {
                        source: &mut source,
                        header: image.catalog().header(),
                        budget: actual.budget,
                        cancelled,
                        reads: 0,
                    },
                    index.usage_map(),
                    [right, left],
                    &mut expected_edits,
                )?;
                actual
                    .budget
                    .charge_allocation(2, size_of::<u32>() as u64)
                    .map_err(InspectError::from)?;
                expected_pages
                    .try_reserve(2)
                    .map_err(|_| InspectError::Allocation)?;
                expected_pages.extend([right, left]);
                next_eof = left.checked_add(1).ok_or(EditError::Preservation)?;
                split_position += 1;
            }
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
            if split && number == index.root_page() {
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
        || plan.added.len() != 2 * split_position
        || next_eof as usize * PAGE_BYTES != plan.length
    {
        return Err(EditError::Preservation);
    }
    for (position, page) in plan.added.iter().enumerate() {
        check(actual.budget, cancelled)?;
        if page.number as usize != plan.original_length / PAGE_BYTES + position
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
    maps::finish(
        &mut Reader {
            source: &mut source,
            header: image.catalog().header(),
            budget: actual.budget,
            cancelled,
            reads: 0,
        },
        &expected_edits,
        &plan.map_edits,
        after,
    )?;
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
