//! Split existing leaves without increasing the checked tree height.
use super::super::{IndexDataPatch, encoding, profile::SingleProfile, repack, tree::Tree};
use super::{AddedPage, Plan, leaf_entries, maps, pages};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, WriteBlocker,
    format::{page::PAGE_BYTES, reader::Reader},
    limits::MAX_IMAGE_BYTES,
    table::indexes::{
        header::IndexHeader,
        page::{Entry, KeyRef},
    },
};
use std::io::Cursor;

const PREVIOUS_OFFSET: usize = std::mem::offset_of!(IndexHeader, previous);
const NEXT_OFFSET: usize = std::mem::offset_of!(IndexHeader, next);

struct Leaf<'a> {
    number: u32,
    entries: &'a [KeyRef],
    left: Option<u32>,
}

struct ResultChild {
    maximum: KeyRef,
    left: Option<(u32, KeyRef)>,
}

pub(super) struct Operation<'a> {
    pub image: &'a NativeImage,
    pub index: &'a NativeIndex,
    pub profile: &'a SingleProfile,
}

pub(super) fn edit(
    operation: &Operation<'_>,
    tree: &Tree,
    keys: &[KeyRef],
    plan: &mut Plan,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    patches: &mut Vec<IndexDataPatch>,
) -> Result<(), EditError> {
    let mut leaves = Vec::new();
    let mut remaining = keys;
    collect(tree, &mut remaining, &mut leaves, reader)?;
    if !remaining.is_empty() {
        return Err(EditError::Preservation);
    }
    for leaf in &mut leaves {
        reader.tick()?;
        let original = raw(operation.image, leaf.number)?;
        let entries = leaf_entries(leaf.entries, reader.budget, reader.cancelled)?;
        if !repack::fits::<_, { encoding::MAX_ENTRY_BYTES }>(
            original,
            &entries,
            operation.profile,
            reader.budget,
            reader.cancelled,
        )? {
            if entries.len() < 2 {
                return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
            }
            let length = plan
                .length
                .checked_add(PAGE_BYTES)
                .filter(|length| *length as u64 <= MAX_IMAGE_BYTES)
                .ok_or(InspectError::StructureLimit)?;
            let number = u32::try_from(plan.length / PAGE_BYTES)
                .map_err(|_| InspectError::StructureLimit)?;
            maps::allocate_nonroot(
                operation.image,
                operation.index,
                number,
                &mut plan.map_edits,
                reader.budget,
                reader.cancelled,
            )?;
            leaf.left = Some(number);
            plan.length = length;
        }
    }
    rewrite(operation, tree, &leaves, plan, reader, patches)?;
    Ok(())
}

fn collect<'a>(
    tree: &Tree,
    remaining: &mut &'a [KeyRef],
    leaves: &mut Vec<Leaf<'a>>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.tick()?;
    if tree.is_leaf() {
        let (entries, rest) = remaining
            .split_at_checked(tree.count())
            .ok_or(EditError::Preservation)?;
        *remaining = rest;
        reader
            .budget
            .charge_allocation(1, size_of::<Leaf<'_>>() as u64)
            .map_err(InspectError::from)?;
        leaves
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        leaves.push(Leaf {
            number: tree.number(),
            entries,
            left: None,
        });
    } else {
        for child in tree.children() {
            collect(child, remaining, leaves, reader)?;
        }
    }
    Ok(())
}

fn rewrite(
    operation: &Operation<'_>,
    tree: &Tree,
    leaves: &[Leaf<'_>],
    plan: &mut Plan,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    patches: &mut Vec<IndexDataPatch>,
) -> Result<ResultChild, EditError> {
    reader.tick()?;
    let original = raw(operation.image, tree.number())?;
    if tree.is_leaf() {
        reader.work(leaves.len())?;
        let position = leaves
            .iter()
            .position(|leaf| leaf.number == tree.number())
            .ok_or(EditError::Preservation)?;
        let leaf = &leaves[position];
        let next = leaves
            .get(position + 1)
            .map(|leaf| leaf.left.unwrap_or(leaf.number))
            .unwrap_or(0);
        let previous = position
            .checked_sub(1)
            .map(|prior| leaves[prior].number)
            .unwrap_or(0);
        let mut entries = leaf.entries;
        let mut left = None;
        if let Some(number) = leaf.left {
            let middle = entries.len() / 2;
            let l = leaf_entries(&entries[..middle], reader.budget, reader.cancelled)?;
            let page = pages::leaf(
                plan.owner,
                previous,
                tree.number(),
                &l,
                operation.profile,
                reader.budget,
                reader.cancelled,
            )?;
            reader
                .budget
                .charge_allocation(1, size_of::<AddedPage>() as u64)
                .map_err(InspectError::from)?;
            plan.added
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            plan.added.push(AddedPage {
                number,
                bytes: page,
            });
            left = Some((number, entries[middle - 1]));
            entries = &entries[middle..];
        }
        let encoded = leaf_entries(entries, reader.budget, reader.cancelled)?;
        let mut after = repack::page::<_, { encoding::MAX_ENTRY_BYTES }>(
            original,
            &encoded,
            operation.profile,
            reader.budget,
            reader.cancelled,
        )?;
        let previous = leaf.left.unwrap_or(previous);
        after[PREVIOUS_OFFSET..PREVIOUS_OFFSET + size_of::<u32>()]
            .copy_from_slice(&previous.to_le_bytes());
        after[NEXT_OFFSET..NEXT_OFFSET + size_of::<u32>()].copy_from_slice(&next.to_le_bytes());
        repack::patches(
            original,
            &after,
            tree.number(),
            patches,
            reader.budget,
            reader.cancelled,
        )?;
        for offset in [PREVIOUS_OFFSET, NEXT_OFFSET] {
            link_patch(original, &after, tree.number(), offset, patches, reader)?;
        }
        return Ok(ResultChild {
            maximum: *entries.last().ok_or(EditError::Preservation)?,
            left,
        });
    }
    let header = IndexHeader::read(original)?;
    let mut entries = Vec::new();
    let mut maximum = None;
    for (position, child) in tree.children().iter().enumerate() {
        let result = rewrite(operation, child, leaves, plan, reader, patches)?;
        if let Some((number, value)) = result.left {
            push(
                &mut entries,
                Entry {
                    value,
                    child: number,
                },
                reader,
            )?;
        }
        if position < tree.count() {
            push(
                &mut entries,
                Entry {
                    value: result.maximum,
                    child: child.number(),
                },
                reader,
            )?;
        } else if header.tail.get() != child.number() {
            return Err(EditError::Preservation);
        }
        maximum = Some(result.maximum);
    }
    // This increment deliberately refuses any parent overflow instead of
    // silently changing height, promoting a child or splitting another codec.
    let after = repack::page::<_, { encoding::MAX_ENTRY_BYTES }>(
        original,
        &entries,
        operation.profile,
        reader.budget,
        reader.cancelled,
    )?;
    repack::patches(
        original,
        &after,
        tree.number(),
        patches,
        reader.budget,
        reader.cancelled,
    )?;
    Ok(ResultChild {
        maximum: maximum.ok_or(EditError::Preservation)?,
        left: None,
    })
}

fn link_patch(
    before: &[u8],
    after: &[u8],
    number: u32,
    offset: usize,
    patches: &mut Vec<IndexDataPatch>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let range = offset..offset + size_of::<u32>();
    reader.work(range.len())?;
    if before[range.clone()] == after[range.clone()] {
        return Ok(());
    }
    reader
        .budget
        .charge_allocation(2 * range.len() as u64, 1)
        .map_err(InspectError::from)?;
    let mut original = Vec::new();
    let mut replacement = Vec::new();
    original
        .try_reserve_exact(range.len())
        .map_err(|_| InspectError::Allocation)?;
    replacement
        .try_reserve_exact(range.len())
        .map_err(|_| InspectError::Allocation)?;
    original.extend_from_slice(&before[range.clone()]);
    replacement.extend_from_slice(&after[range]);
    reader
        .budget
        .charge_allocation(1, size_of::<IndexDataPatch>() as u64)
        .map_err(InspectError::from)?;
    patches
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    patches.push(IndexDataPatch {
        offset: number as usize * PAGE_BYTES + offset,
        before: original,
        after: replacement,
    });
    Ok(())
}

fn push(
    entries: &mut Vec<Entry>,
    entry: Entry,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader
        .budget
        .charge_allocation(1, size_of::<Entry>() as u64)
        .map_err(InspectError::from)?;
    entries
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    entries.push(entry);
    Ok(())
}

fn raw(image: &NativeImage, number: u32) -> Result<&[u8], EditError> {
    let offset = number as usize * PAGE_BYTES;
    image
        .as_bytes()
        .get(offset..offset + PAGE_BYTES)
        .ok_or_else(|| InspectError::Malformed.into())
}
