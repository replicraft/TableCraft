//! Cardinality-growing height-one INSERT with a preflighted symbolic layout.
mod layout;

use super::super::{AllocationPolicy, Plan, allocation, leaf_entries, pages, seal};
use super::{ENTRY_BUFFER, Operation, primary, prior_allocations, retained_members};
use crate::{
    EditError, InspectError, NativeImage, WriteBlocker,
    format::{page::PAGE_BYTES, reader::Reader},
    header::check,
    limits::{MAX_IMAGE_BYTES, MAX_LIVE_ROWS},
    table::{
        indexes::{
            header::IndexHeader,
            mutation::{IndexDataPatch, NumericKey, profile::SingleProfile, repack, tree::Tree},
            page::{self, NODE},
        },
        mutation::{
            append::State,
            map_authority::{self, Owner},
        },
    },
};
use std::{io::Cursor, ops::Range};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

const PREVIOUS_OFFSET: usize = std::mem::offset_of!(IndexHeader, previous);
const NEXT_OFFSET: usize = std::mem::offset_of!(IndexHeader, next);
const TAIL_OFFSET: usize = std::mem::offset_of!(IndexHeader, tail);

/// Reuse fitting children or normalize overflowing routed ranges before any
/// reservation. The same DATA cursor owns every subsequent INDEX/bitmap page.
pub(super) fn continue_primary(
    operation: Operation<'_>,
    append: State,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Plan, EditError> {
    check(budget, cancelled)?;
    let image = operation.image;
    let checked = operation.checked;
    let definition = checked.schema().definition_page();
    let index = primary(&operation, budget, cancelled)?;
    if append.original_length != image.as_bytes().len()
        || append.owner != definition
        || append.length < append.original_length
        || append.length as u64 > MAX_IMAGE_BYTES
        || !append.length.is_multiple_of(PAGE_BYTES)
        || (append.length - append.original_length) / PAGE_BYTES != append.reservations_len()
        || append.indexes().next().is_some()
    {
        return Err(EditError::Preservation);
    }
    append.preview(0)?;
    if operation.final_keys.len() > MAX_LIVE_ROWS {
        return Err(InspectError::StructureLimit.into());
    }
    let profile = SingleProfile {
        column: checked.schema().columns()[operation.id].number(),
        kind: NumericKey::new(ScalarType::Int32)?,
        descending: false,
    };
    let root = raw(image, index.root_page())?;
    let root_header = IndexHeader::read(root)?;
    if root_header.kind != NODE
        || !root_header.qualified_for_update(1)
        || root_header.previous.get() != 0
        || root_header.next.get() != 0
    {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let mut original = Vec::new();
    let tree = Tree::read::<_, ENTRY_BUFFER>(
        image,
        &mut reader,
        definition,
        index.root_page(),
        &profile,
        0,
        &mut original,
    )?;
    if tree.height() != 1
        || tree.children().is_empty()
        || tree.children().iter().any(|child| !child.is_leaf())
        || original.len() != checked.rows()
    {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let decoded = page::read_profile::<_, _, ENTRY_BUFFER>(
        &mut reader,
        index.root_page(),
        definition,
        &profile,
    )?;
    let tail = decoded.tail != 0;
    if decoded.entries.len() + usize::from(tail) != tree.children().len() {
        return Err(EditError::Preservation);
    }
    let mut original_end = 0_usize;
    for (position, child) in tree.children().iter().enumerate() {
        reader.tick()?;
        original_end = original_end
            .checked_add(child.count())
            .ok_or(InspectError::StructureLimit)?;
        let bound = original
            .get(original_end.checked_sub(1).ok_or(EditError::Preservation)?)
            .ok_or(EditError::Preservation)?;
        if let Some(entry) = decoded.entries.get(position) {
            if entry.value != *bound || entry.child != child.number() {
                return Err(EditError::Preservation);
            }
        } else if !tail || decoded.tail != child.number() {
            return Err(EditError::Preservation);
        }
        let header = IndexHeader::read(raw(image, child.number())?)?;
        let previous = position
            .checked_sub(1)
            .map(|prior| tree.children()[prior].number())
            .unwrap_or(0);
        let next = tree
            .children()
            .get(position + 1)
            .map(Tree::number)
            .unwrap_or(0);
        if header.previous.get() != previous || header.next.get() != next || header.tail.get() != 0
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    if operation.final_keys.len() == original.len() {
        reader.work(original.len())?;
        if operation.final_keys != original {
            return Err(EditError::Conflict);
        }
    } else {
        retained_members(&original, operation.final_keys, &append, &mut reader)?;
    }
    let mut ranges: Vec<Range<usize>> = Vec::new();
    layout::reserve(&mut ranges, tree.children().len(), &mut reader)?;
    let mut start = 0;
    for position in 0..tree.children().len() {
        let mut end = start;
        if position + 1 == tree.children().len() {
            end = operation.final_keys.len();
        } else {
            let bound = decoded
                .entries
                .get(position)
                .ok_or(EditError::Preservation)?
                .value;
            while let Some(entry) = operation.final_keys.get(end) {
                reader.tick()?;
                if entry.compare_with(&bound, &profile).is_gt() {
                    break;
                }
                end += 1;
            }
        }
        if end == start {
            return Err(EditError::Preservation);
        }
        ranges.push(start..end);
        start = end;
    }
    let layout = layout::prepare(
        image,
        &tree,
        &ranges,
        operation.final_keys,
        &profile,
        &mut reader,
    )?;

    let census =
        map_authority::capture_catalog(image, reader.budget, cancelled, |_, _, _, _| Ok(()))?;
    census.require_exclusive(
        &[
            Owner::GlobalFree,
            Owner::OrdinaryOwned { definition },
            Owner::OrdinaryCandidate { definition },
            Owner::IndexOwned {
                definition,
                physical: index.data_number(),
            },
        ],
        reader.budget,
        cancelled,
    )?;
    let mut owned = crate::format::allocation::pages(&mut reader, index.usage_map())?;
    let mut expected_owned = Vec::new();
    layout::reserve(&mut expected_owned, tree.children().len() + 1, &mut reader)?;
    expected_owned.push(index.root_page());
    for child in tree.children() {
        reader.tick()?;
        expected_owned.push(child.number());
    }
    let work = expected_owned
        .len()
        .checked_mul(expected_owned.len())
        .ok_or(InspectError::StructureLimit)?;
    reader.work(work)?;
    owned.sort_unstable();
    expected_owned.sort_unstable();
    if owned != expected_owned {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    prior_allocations(&append, &mut reader)?;
    let mut plan = Plan::with_policy(append, AllocationPolicy::Qualified);
    let mut numbers = Vec::new();
    layout::reserve(&mut numbers, layout.leaves.len(), &mut reader)?;
    numbers.extend(layout.leaves.iter().map(|leaf| match leaf.origin {
        layout::Origin::Existing(number) => number,
        layout::Origin::Fresh => 0,
    }));
    // Only final surviving Fresh symbols receive real reservations, in creation
    // order. Repeated symbolic divisions never leave allocated orphan pages.
    for &fresh in &layout.fresh {
        reader.tick()?;
        numbers[fresh] = allocation::reserve(image, index, &mut plan, &mut reader)?;
    }
    let entries = layout::bounds(&layout, operation.final_keys, &numbers, &mut reader)?;
    let mut root_after = repack::borrowed_page::<_, ENTRY_BUFFER>(
        root,
        &entries,
        &profile,
        reader.budget,
        cancelled,
    )?;
    let last = *layout.order.last().ok_or(EditError::Preservation)?;
    let tail = if layout.tail { numbers[last] } else { 0 };
    root_after[TAIL_OFFSET..TAIL_OFFSET + size_of::<u32>()].copy_from_slice(&tail.to_le_bytes());
    patch(index.root_page(), root, root_after, &mut plan, &mut reader)?;
    let mut positions = Vec::new();
    layout::reserve(&mut positions, layout.leaves.len(), &mut reader)?;
    positions.resize(layout.leaves.len(), usize::MAX);
    for (position, &leaf) in layout.order.iter().enumerate() {
        reader.tick()?;
        let stored = positions.get_mut(leaf).ok_or(EditError::Preservation)?;
        if *stored != usize::MAX {
            return Err(EditError::Preservation);
        }
        *stored = position;
    }
    for &existing in &layout.original {
        reader.tick()?;
        let number = numbers[existing];
        let before = raw(image, number)?;
        let leaf = &layout.leaves[existing];
        let keys = operation
            .final_keys
            .get(leaf.range.clone())
            .ok_or(EditError::Preservation)?;
        let entries = leaf_entries(keys, reader.budget, cancelled)?;
        let mut after = repack::borrowed_page::<_, ENTRY_BUFFER>(
            before,
            &entries,
            &profile,
            reader.budget,
            cancelled,
        )?;
        let position = positions[existing];
        let previous = position
            .checked_sub(1)
            .map(|prior| numbers[layout.order[prior]])
            .unwrap_or(0);
        let next = layout
            .order
            .get(position + 1)
            .map(|&next| numbers[next])
            .unwrap_or(0);
        after[PREVIOUS_OFFSET..PREVIOUS_OFFSET + size_of::<u32>()]
            .copy_from_slice(&previous.to_le_bytes());
        after[NEXT_OFFSET..NEXT_OFFSET + size_of::<u32>()].copy_from_slice(&next.to_le_bytes());
        patch(number, before, after, &mut plan, &mut reader)?;
    }
    for &fresh in &layout.fresh {
        reader.tick()?;
        let position = positions[fresh];
        let previous = position
            .checked_sub(1)
            .map(|prior| numbers[layout.order[prior]])
            .unwrap_or(0);
        let next = layout
            .order
            .get(position + 1)
            .map(|&next| numbers[next])
            .unwrap_or(0);
        let keys = operation
            .final_keys
            .get(layout.leaves[fresh].range.clone())
            .ok_or(EditError::Preservation)?;
        let entries = leaf_entries(keys, reader.budget, cancelled)?;
        let bytes = pages::leaf::<_, ENTRY_BUFFER>(
            definition,
            previous,
            next,
            &entries,
            &profile,
            reader.budget,
            cancelled,
        )?;
        layout::reserve(&mut plan.added, 1, &mut reader)?;
        plan.added.push(super::super::AddedPage {
            number: numbers[fresh],
            bytes,
        });
    }
    seal(
        image,
        checked.indexes(),
        &mut plan,
        reader.budget,
        cancelled,
    )?;
    check(reader.budget, cancelled)?;
    Ok(plan)
}

fn raw(image: &NativeImage, number: u32) -> Result<&[u8], EditError> {
    let start = (number as usize)
        .checked_mul(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let end = start
        .checked_add(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    image
        .as_bytes()
        .get(start..end)
        .ok_or(InspectError::Truncated.into())
}

fn patch(
    number: u32,
    before: &[u8],
    after: Vec<u8>,
    plan: &mut Plan,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.work(PAGE_BYTES)?;
    if before == after {
        return Ok(());
    }
    reader
        .budget
        .charge_allocation(PAGE_BYTES as u64, 1)
        .map_err(InspectError::from)?;
    let mut original = Vec::new();
    original
        .try_reserve_exact(PAGE_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    original.extend_from_slice(before);
    reader
        .budget
        .charge_allocation(1, size_of::<IndexDataPatch>() as u64)
        .map_err(InspectError::from)?;
    plan.patches
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    plan.patches.push(IndexDataPatch {
        offset: number as usize * PAGE_BYTES,
        before: original,
        after,
    });
    Ok(())
}

#[cfg(test)]
mod tests;
