//! One cardinality-growing child split under an original height-one primary.
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
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

const PREVIOUS_OFFSET: usize = std::mem::offset_of!(IndexHeader, previous);
const NEXT_OFFSET: usize = std::mem::offset_of!(IndexHeader, next);
const TAIL_OFFSET: usize = std::mem::offset_of!(IndexHeader, tail);

/// Split exactly one original child once. DATA placement and counter authority
/// belong to the INSERT coordinator; no UPDATE cardinality assumptions apply.
pub(in crate::table::indexes) fn continue_primary(
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
        || root_header.tail.get() == 0
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
        || tree.count() != 1
        || tree.children().len() != 2
        || tree.children().iter().any(|child| !child.is_leaf())
        || original.len() != checked.rows()
    {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let left = &tree.children()[0];
    let right = &tree.children()[1];
    let bound = original
        .get(left.count().checked_sub(1).ok_or(EditError::Preservation)?)
        .ok_or(EditError::Preservation)?;
    let decoded = page::read_profile::<_, _, ENTRY_BUFFER>(
        &mut reader,
        index.root_page(),
        definition,
        &profile,
    )?;
    if decoded.entries.len() != 1
        || decoded.entries[0].value != *bound
        || decoded.entries[0].child != left.number()
        || decoded.tail != right.number()
    {
        return Err(EditError::Preservation);
    }
    for (child, previous, next) in [(left, 0, right.number()), (right, left.number(), 0)] {
        let header = IndexHeader::read(raw(image, child.number())?)?;
        if header.previous.get() != previous || header.next.get() != next || header.tail.get() != 0
        {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    retained_members(&original, operation.final_keys, &append, &mut reader)?;
    let mut boundary = 0;
    for entry in operation.final_keys {
        reader.tick()?;
        if !entry.compare_with(bound, &profile).is_gt() {
            boundary += 1;
        }
    }
    if boundary == 0 || boundary == operation.final_keys.len() {
        return Err(EditError::Preservation);
    }
    let partitions = [
        &operation.final_keys[..boundary],
        &operation.final_keys[boundary..],
    ];
    let left_entries = leaf_entries(partitions[0], reader.budget, cancelled)?;
    let right_entries = leaf_entries(partitions[1], reader.budget, cancelled)?;
    let mut fits = [false; 2];
    for (position, (child, entries)) in [(left, &left_entries), (right, &right_entries)]
        .into_iter()
        .enumerate()
    {
        fits[position] = repack::borrowed_fits::<_, ENTRY_BUFFER>(
            raw(image, child.number())?,
            entries,
            &profile,
            reader.budget,
            cancelled,
        )?;
    }
    if fits[0] == fits[1] {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let target = usize::from(fits[0]);
    let target_page = if target == 0 {
        left.number()
    } else {
        right.number()
    };
    let middle = partitions[target].len() / 2;
    if middle == 0 {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let fresh_entries = leaf_entries(&partitions[target][..middle], reader.budget, cancelled)?;
    let retained_entries = leaf_entries(&partitions[target][middle..], reader.budget, cancelled)?;
    if !repack::borrowed_fits_fresh::<_, ENTRY_BUFFER>(
        &fresh_entries,
        &profile,
        reader.budget,
        cancelled,
    )? || !repack::borrowed_fits::<_, ENTRY_BUFFER>(
        raw(image, target_page)?,
        &retained_entries,
        &profile,
        reader.budget,
        cancelled,
    )? {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let new_bound = partitions[target][middle - 1];
    let final_left_bound = *partitions[0].last().ok_or(EditError::Preservation)?;
    let root_keys = if target == 0 {
        [new_bound, final_left_bound]
    } else {
        [final_left_bound, new_bound]
    };
    let mut root_entries = leaf_entries(&root_keys, reader.budget, cancelled)?;
    // Child identities occupy a fixed-width field. A nonzero stand-in permits
    // complete root capacity preflight before any metadata or index reservation.
    for entry in &mut root_entries {
        entry.child = target_page;
    }
    if !repack::borrowed_fits::<_, ENTRY_BUFFER>(
        root,
        &root_entries,
        &profile,
        reader.budget,
        cancelled,
    )? {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }

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
    let mut expected_owned = [index.root_page(), left.number(), right.number()];
    owned.sort_unstable();
    expected_owned.sort_unstable();
    if owned != expected_owned {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    prior_allocations(&append, &mut reader)?;
    let mut plan = Plan::with_policy(append, AllocationPolicy::Qualified);
    let fresh = allocation::reserve(image, index, &mut plan, &mut reader)?;
    let order = if target == 0 {
        [fresh, left.number(), right.number()]
    } else {
        [left.number(), fresh, right.number()]
    };
    root_entries[0].child = order[0];
    root_entries[1].child = order[1];
    let mut root_after = repack::borrowed_page::<_, ENTRY_BUFFER>(
        root,
        &root_entries,
        &profile,
        reader.budget,
        cancelled,
    )?;
    root_after[TAIL_OFFSET..TAIL_OFFSET + size_of::<u32>()]
        .copy_from_slice(&order[2].to_le_bytes());
    patch(index.root_page(), root, root_after, &mut plan, &mut reader)?;
    for (position, child) in [left, right].into_iter().enumerate() {
        let entries = if position == target {
            &retained_entries
        } else if position == 0 {
            &left_entries
        } else {
            &right_entries
        };
        let before = raw(image, child.number())?;
        let mut after = repack::borrowed_page::<_, ENTRY_BUFFER>(
            before,
            entries,
            &profile,
            reader.budget,
            cancelled,
        )?;
        let slot = order
            .iter()
            .position(|number| *number == child.number())
            .ok_or(EditError::Preservation)?;
        let previous = slot.checked_sub(1).map(|prior| order[prior]).unwrap_or(0);
        let next = order.get(slot + 1).copied().unwrap_or(0);
        after[PREVIOUS_OFFSET..PREVIOUS_OFFSET + size_of::<u32>()]
            .copy_from_slice(&previous.to_le_bytes());
        after[NEXT_OFFSET..NEXT_OFFSET + size_of::<u32>()].copy_from_slice(&next.to_le_bytes());
        patch(child.number(), before, after, &mut plan, &mut reader)?;
    }
    let fresh_slot = order
        .iter()
        .position(|number| *number == fresh)
        .ok_or(EditError::Preservation)?;
    let previous = fresh_slot
        .checked_sub(1)
        .map(|prior| order[prior])
        .unwrap_or(0);
    let bytes = pages::leaf::<_, ENTRY_BUFFER>(
        definition,
        previous,
        target_page,
        &fresh_entries,
        &profile,
        reader.budget,
        cancelled,
    )?;
    reader
        .budget
        .charge_allocation(1, size_of::<super::super::AddedPage>() as u64)
        .map_err(InspectError::from)?;
    plan.added
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    plan.added.push(super::super::AddedPage {
        number: fresh,
        bytes,
    });
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
