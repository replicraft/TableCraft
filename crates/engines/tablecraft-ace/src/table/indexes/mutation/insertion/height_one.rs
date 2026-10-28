//! Cardinality growth inside the two retained leaves of an original height-one
//! primary. Original separators route INSERTs; UPDATE repartitioning is separate.
use super::super::{IndexDataPatch, profile::SingleProfile, repack, split, tree::Tree};
use super::{ENTRY_BUFFER, qualify};
use crate::{
    EditError, InspectError, NativeImage, TableCheck, WriteBlocker,
    format::{
        page::PAGE_BYTES,
        reader::Reader,
        row::{MAX_SLOTS, reference_page, reference_slot},
    },
    header::check,
    limits::{MAX_IMAGE_BYTES, MAX_LIVE_ROWS},
    table::indexes::{
        header::IndexHeader,
        mutation::NumericKey,
        page::{self, KeyRef, NODE},
    },
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

/// DATA placement and counters remain independently certified by the caller.
/// This helper neither borrows nor changes the shared append cursor.
/// `None` reports capacity exhaustion only; structural errors remain failures.
pub(in crate::table::indexes) fn prepare(
    image: &NativeImage,
    checked: &TableCheck,
    id: usize,
    final_keys: &[KeyRef],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<Vec<IndexDataPatch>>, EditError> {
    let index = qualify(checked, id, budget, cancelled)?;
    let profile = SingleProfile {
        column: checked.schema().columns()[id].number(),
        kind: NumericKey::new(ScalarType::Int32)?,
        descending: false,
    };
    let root = raw(image, index.root_page())?;
    let header = IndexHeader::read(root)?;
    if header.kind != NODE
        || !header.qualified_for_update(1)
        || header.previous.get() != 0
        || header.next.get() != 0
        || header.tail.get() == 0
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
    let mut original_keys = Vec::new();
    let tree = Tree::read::<_, ENTRY_BUFFER>(
        image,
        &mut reader,
        checked.schema().definition_page(),
        index.root_page(),
        &profile,
        0,
        &mut original_keys,
    )?;
    if tree.height() != 1
        || tree.count() != 1
        || tree.children().len() != 2
        || tree.children().iter().any(|child| !child.is_leaf())
        || original_keys.len() != checked.rows()
    {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let left = &tree.children()[0];
    let right = &tree.children()[1];
    let boundary = original_keys
        .get(left.count().checked_sub(1).ok_or(EditError::Preservation)?)
        .ok_or(EditError::Preservation)?;
    let decoded = page::read_profile::<_, _, ENTRY_BUFFER>(
        &mut reader,
        index.root_page(),
        checked.schema().definition_page(),
        &profile,
    )?;
    if decoded.entries.len() != 1
        || decoded.entries[0].value != *boundary
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

    if final_keys.len() < original_keys.len() || final_keys.len() > MAX_LIVE_ROWS {
        return Err(EditError::Preservation);
    }
    let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
    let mut previous = None;
    let mut original_position = 0;
    let mut split_at = 0;
    for (position, entry) in final_keys.iter().enumerate() {
        reader.tick()?;
        if entry.key.and_then(|key| i32::try_from(key).ok()).is_none() {
            return Err(EditError::Unsupported(WriteBlocker::Column));
        }
        let page = reference_page(entry.row);
        if page == 0
            || u64::from(page) >= MAX_IMAGE_BYTES / PAGE_BYTES as u64
            || usize::from(reference_slot(entry.row)) >= MAX_SLOTS
        {
            return Err(EditError::Conflict);
        }
        if position != 0
            && !final_keys[position - 1]
                .compare_with(entry, &profile)
                .is_lt()
        {
            return Err(EditError::Conflict);
        }
        constraints
            .check_tuple(&mut previous, entry.key, false)
            .map_err(InspectError::from)?;
        reader.work(position)?;
        if final_keys[..position]
            .iter()
            .any(|old| old.row == entry.row)
        {
            return Err(EditError::Conflict);
        }
        if let Some(old) = original_keys.get(original_position) {
            match old.compare_with(entry, &profile) {
                std::cmp::Ordering::Less => return Err(EditError::Preservation),
                std::cmp::Ordering::Equal => original_position += 1,
                std::cmp::Ordering::Greater => {}
            }
        }
        if !entry.compare_with(boundary, &profile).is_gt() {
            split_at = position + 1;
        }
    }
    if original_position != original_keys.len() || split_at == 0 || split_at == final_keys.len() {
        return Err(EditError::Preservation);
    }

    let partitions = [&final_keys[..split_at], &final_keys[split_at..]];
    let left_entries = split::leaf_entries(partitions[0], reader.budget, cancelled)?;
    let right_entries = split::leaf_entries(partitions[1], reader.budget, cancelled)?;
    // Complete both capacity decisions before constructing any returned patch.
    for (child, entries) in [(left, &left_entries), (right, &right_entries)] {
        if !repack::borrowed_fits::<_, ENTRY_BUFFER>(
            raw(image, child.number())?,
            entries,
            &profile,
            reader.budget,
            cancelled,
        )? {
            return Ok(None);
        }
    }
    let mut patches = Vec::new();
    let mut original_start = 0;
    for (position, (child, entries)) in [(left, &left_entries), (right, &right_entries)]
        .into_iter()
        .enumerate()
    {
        let original_end = original_start + child.count();
        if partitions[position] != &original_keys[original_start..original_end] {
            let before = raw(image, child.number())?;
            let after = repack::borrowed_page::<_, ENTRY_BUFFER>(
                before,
                entries,
                &profile,
                reader.budget,
                cancelled,
            )?;
            repack::patches(
                before,
                &after,
                child.number(),
                &mut patches,
                reader.budget,
                cancelled,
            )?;
        }
        original_start = original_end;
    }
    check(reader.budget, cancelled)?;
    Ok(Some(patches))
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

#[cfg(test)]
mod tests;
