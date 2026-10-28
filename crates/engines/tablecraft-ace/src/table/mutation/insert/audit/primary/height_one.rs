//! Independent INSERT proof for one original bound and two old leaves.
mod split;
#[cfg(test)]
mod tests;

use super::super::{ImageProof, reserve};
use super::{
    BITMAP_BITS, BITMAP_START, FREE_OFFSET, INDEX_VERSION, KEY_MARKER, PREFIX_OFFSET, WIRE_BYTES,
    decode, page_range, reference, render, value,
};
use crate::{
    EditError, NativeImage, NativeIndex,
    format::{allocation, page::PAGE_BYTES, reader::Reader},
    table::indexes::{
        header::IndexHeader,
        mutation::insertion::PrimaryInsertion,
        page::{ENTRY_START, LEAF, NODE},
    },
};
use std::io::Cursor;

const ROOT_HEIGHT: u8 = 1;
const LEAF_HEIGHT: u8 = 0;
const CHILD_BYTES: usize = size_of::<u32>();
const BOUND_BYTES: usize = WIRE_BYTES + CHILD_BYTES;
const CAPACITY: usize = PAGE_BYTES - ENTRY_START;
const CHILDREN: usize = 2;

struct Source {
    root: u32,
    leaves: [Leaf; CHILDREN],
    bound: [u8; WIRE_BYTES],
    entries: Vec<[u8; WIRE_BYTES]>,
}

struct Leaf {
    number: u32,
    count: usize,
    active: usize,
    changed: bool,
}

pub(super) enum Change {
    Fitted(Fitting),
    Split(split::Growth),
}

pub(super) struct Fitting {
    root: u32,
    leaves: [Leaf; CHILDREN],
}

pub(super) fn original(
    image: &NativeImage,
    index: &NativeIndex,
    definition: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<[u8; WIRE_BYTES]>, EditError> {
    Ok(Source::capture(image, index, definition, reader)?.entries)
}

pub(super) fn derive(
    image: &NativeImage,
    index: &NativeIndex,
    definition: u32,
    entries: &[[u8; WIRE_BYTES]],
    proof: &mut ImageProof<'_>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Change, EditError> {
    reader.tick()?;
    if !std::ptr::eq(proof.original, image.as_bytes())
        || proof.bytes.len() < proof.original.len()
        || !proof.bytes.len().is_multiple_of(PAGE_BYTES)
    {
        return Err(EditError::Preservation);
    }

    let mut source = Source::capture(image, index, definition, reader)?;
    membership(&source.entries, entries, reader)?;
    let mut boundary = 0;
    for entry in entries {
        reader.tick()?;
        if entry <= &source.bound {
            boundary += 1;
        }
    }
    if boundary == 0 || boundary == entries.len() {
        return Err(EditError::Preservation);
    }
    let partitions = [&entries[..boundary], &entries[boundary..]];

    // Preflight both original-seeded capacities before changing expected bytes.
    // The independent renderer is owned by this auditor, not the key planner.
    let mut rendered = Vec::new();
    reserve(&mut rendered, CHILDREN, reader)?;
    for (leaf, partition) in source.leaves.iter().zip(partitions) {
        let range = page_range(leaf.number)?;
        let before = image.as_bytes().get(range).ok_or(EditError::Preservation)?;
        rendered.push(render(before, partition, reader)?);
    }
    if rendered.iter().any(Option::is_none) {
        return split::derive(image, index, source, entries, proof, reader).map(Change::Split);
    }

    let root_range = page_range(source.root)?;
    reader.work(PAGE_BYTES)?;
    if proof.bytes.get(root_range.clone()) != image.as_bytes().get(root_range) {
        return Err(EditError::Preservation);
    }
    let mut start: usize = 0;
    for ((leaf, partition), rendered) in source.leaves.iter_mut().zip(partitions).zip(rendered) {
        let (after, active) = rendered.ok_or(EditError::Preservation)?;
        let end = start
            .checked_add(leaf.count)
            .ok_or(EditError::Preservation)?;
        let before_entries = source
            .entries
            .get(start..end)
            .ok_or(EditError::Preservation)?;
        reader.work(partition.len().max(before_entries.len()) * WIRE_BYTES)?;
        leaf.changed = partition != before_entries;
        start = end;

        let range = page_range(leaf.number)?;
        reader.work(PAGE_BYTES)?;
        if proof.bytes.get(range.clone()) != image.as_bytes().get(range.clone()) {
            return Err(EditError::Preservation);
        }
        if leaf.changed {
            leaf.active = leaf.active.max(active);
            proof.bytes[range].copy_from_slice(&after);
        }
    }
    if start != source.entries.len() {
        return Err(EditError::Preservation);
    }

    Ok(Change::Fitted(Fitting {
        root: source.root,
        leaves: source.leaves,
    }))
}

impl Change {
    pub(super) fn compare(
        &self,
        actual: &PrimaryInsertion,
        proof: &ImageProof<'_>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        match self {
            Self::Fitted(fitting) => fitting.compare(actual, proof, reader),
            Self::Split(growth) => growth.compare(actual, proof, reader),
        }
    }

    pub(super) fn growth(&self) -> Option<&split::Growth> {
        match self {
            Self::Fitted(_) => None,
            Self::Split(growth) => Some(growth),
        }
    }
}

impl Source {
    fn capture(
        image: &NativeImage,
        index: &NativeIndex,
        definition: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Self, EditError> {
        let root = raw(image, index.root_page())?;
        let header = IndexHeader::read(root)?;
        if header.kind != NODE
            || header.version != INDEX_VERSION
            || header.owner.get() != definition
            || !header.qualified_for_update(ROOT_HEIGHT)
            || header.previous.get() != 0
            || header.next.get() != 0
            || header.tail.get() == 0
            || usize::from(header.prefix.get()) > BOUND_BYTES
            || usize::from(header.free.get()) != CAPACITY - BOUND_BYTES
        {
            return Err(EditError::Preservation);
        }

        reader.work((ENTRY_START - BITMAP_START) * BITMAP_BITS)?;
        for endpoint in 0..(ENTRY_START - BITMAP_START) * BITMAP_BITS {
            let set =
                root[BITMAP_START + endpoint / BITMAP_BITS] & (1 << (endpoint % BITMAP_BITS)) != 0;
            if set != (endpoint == BOUND_BYTES) {
                return Err(EditError::Preservation);
            }
        }
        let mut bound = [0; WIRE_BYTES];
        bound.copy_from_slice(&root[ENTRY_START..ENTRY_START + WIRE_BYTES]);
        let child = root[ENTRY_START + WIRE_BYTES..ENTRY_START + BOUND_BYTES]
            .try_into()
            .map_err(|_| EditError::Preservation)?;
        let left = u32::from_be_bytes(child);
        let right = header.tail.get();
        if bound[0] != KEY_MARKER
            || left == 0
            || left == right
            || left == index.root_page()
            || right == index.root_page()
        {
            return Err(EditError::Preservation);
        }

        let mut expected = [index.root_page(), left, right];
        expected.sort_unstable();
        let members = allocation::pages(reader, index.usage_map())?;
        if members != expected {
            return Err(EditError::Preservation);
        }
        let mut entries = Vec::new();
        let mut leaves = Vec::new();
        reserve(&mut leaves, CHILDREN, reader)?;
        for (number, previous, next) in [(left, 0, right), (right, left, 0)] {
            let bytes = raw(image, number)?;
            let header = IndexHeader::read(bytes)?;
            if header.kind != LEAF
                || header.version != INDEX_VERSION
                || header.owner.get() != definition
                || !header.qualified_for_update(LEAF_HEIGHT)
                || header.previous.get() != previous
                || header.next.get() != next
                || header.tail.get() != 0
            {
                return Err(EditError::Preservation);
            }
            let (keys, active) = decode(bytes, reader)?;
            if keys.is_empty() {
                return Err(EditError::Preservation);
            }
            leaves.push(Leaf {
                number,
                count: keys.len(),
                active,
                changed: false,
            });
            reserve(&mut entries, keys.len(), reader)?;
            entries.extend(keys);
        }
        let leaves: [Leaf; CHILDREN] = leaves.try_into().map_err(|_| EditError::Preservation)?;
        if entries.get(leaves[0].count - 1) != Some(&bound) {
            return Err(EditError::Preservation);
        }
        membership(&entries, &entries, reader)?;

        Ok(Self {
            root: index.root_page(),
            leaves,
            bound,
            entries,
        })
    }
}

impl Fitting {
    pub(super) fn compare(
        &self,
        actual: &PrimaryInsertion,
        proof: &ImageProof<'_>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        if !actual.added.is_empty() || !actual.promotions.is_empty() || !actual.rebases.is_empty() {
            return Err(EditError::Preservation);
        }
        let range = page_range(self.root)?;
        reader.work(PAGE_BYTES)?;
        if proof.bytes.get(range.clone()) != proof.original.get(range) {
            return Err(EditError::Preservation);
        }

        let mut position = 0;
        for leaf in &self.leaves {
            let page = page_range(leaf.number)?;
            let before = proof
                .original
                .get(page.clone())
                .ok_or(EditError::Preservation)?;
            let after = proof
                .bytes
                .get(page.clone())
                .ok_or(EditError::Preservation)?;
            if !leaf.changed {
                reader.work(PAGE_BYTES)?;
                if before != after {
                    return Err(EditError::Preservation);
                }
                continue;
            }
            for range in [
                FREE_OFFSET..FREE_OFFSET + size_of::<u16>(),
                PREFIX_OFFSET..PREFIX_OFFSET + size_of::<u16>(),
                BITMAP_START..ENTRY_START + leaf.active,
            ] {
                reader.work(range.len())?;
                if before[range.clone()] == after[range.clone()] {
                    continue;
                }
                let patch = actual
                    .patches
                    .get(position)
                    .ok_or(EditError::Preservation)?;
                if patch.offset != page.start + range.start
                    || patch.before != before[range.clone()]
                    || patch.after != after[range]
                {
                    return Err(EditError::Preservation);
                }
                position += 1;
            }
        }
        if position != actual.patches.len() {
            return Err(EditError::Preservation);
        }

        Ok(())
    }
}

fn raw(image: &NativeImage, number: u32) -> Result<&[u8], EditError> {
    image
        .as_bytes()
        .get(page_range(number)?)
        .ok_or(EditError::Preservation)
}

fn membership(
    original: &[[u8; WIRE_BYTES]],
    entries: &[[u8; WIRE_BYTES]],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    if entries.len() < original.len() {
        return Err(EditError::Preservation);
    }
    for (position, entry) in entries.iter().enumerate() {
        reader.work(position + 1)?;
        if entry[0] != KEY_MARKER
            || (position > 0 && value(&entries[position - 1]) >= value(entry))
            || entries[..position]
                .iter()
                .any(|prior| reference(prior) == reference(entry))
        {
            return Err(EditError::Preservation);
        }
    }
    for entry in original {
        reader.work(entries.len())?;
        if entries.binary_search(entry).is_err() {
            return Err(EditError::Preservation);
        }
    }

    Ok(())
}
