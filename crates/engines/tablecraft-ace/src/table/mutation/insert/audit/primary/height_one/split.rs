//! Raw one-child INSERT split replay; no builder partition or arena authority.
#[cfg(test)]
mod tests;

use super::super::super::{ImageProof, copied, reserve};
use super::super::{
    INDEX_VERSION, KEY_MARKER, WIRE_BYTES, page_range, render,
    split::{PromotionExpected, maps::Allocation},
};
use super::{Source, membership, raw};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex,
    format::{page::PAGE_BYTES, reader::Reader},
    table::indexes::{
        header::IndexHeader,
        mutation::insertion::PrimaryInsertion,
        page::{ENTRY_START, LEAF},
    },
};
use std::{io::Cursor, mem::offset_of};

const CHILD_BYTES: usize = size_of::<u32>();
const BOUND_BYTES: usize = WIRE_BYTES + CHILD_BYTES;
const MASK_START: usize = size_of::<IndexHeader>();
const FREE: usize = offset_of!(IndexHeader, free);
const PREFIX: usize = offset_of!(IndexHeader, prefix);
const OWNER: usize = offset_of!(IndexHeader, owner);
const PREVIOUS: usize = offset_of!(IndexHeader, previous);
const NEXT: usize = offset_of!(IndexHeader, next);
const BITS: usize = u8::BITS as usize;
const CAPACITY: usize = PAGE_BYTES - ENTRY_START;
const SOURCE_PAGES: usize = 3;

pub(in crate::table::mutation::insert::audit::primary) struct Growth {
    pub new_left: u32,
    pub promoted: Option<PromotionExpected>,
    pages: [u32; SOURCE_PAGES],
    begin: u32,
    physical: u32,
    usage: u32,
    rebased: Option<u32>,
}

pub(super) fn derive(
    image: &NativeImage,
    index: &NativeIndex,
    source: Source,
    entries: &[[u8; WIRE_BYTES]],
    proof: &mut ImageProof<'_>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Growth, EditError> {
    reader.tick()?;
    if !std::ptr::eq(proof.original, image.as_bytes())
        || proof.bytes.len() < proof.original.len()
        || !proof.bytes.len().is_multiple_of(PAGE_BYTES)
    {
        return Err(EditError::Preservation);
    }
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
    let left_number = source.leaves[0].number;
    let right_number = source.leaves[1].number;
    let root = raw(image, source.root)?;
    let definition = IndexHeader::read(root)?.owner.get();

    let mut rendered = Vec::new();
    reserve(&mut rendered, 2, reader)?;
    for (leaf, partition) in source.leaves.iter().zip(partitions) {
        rendered.push(render(raw(image, leaf.number)?, partition, reader)?);
    }
    let target = match (rendered[0].is_none(), rendered[1].is_none()) {
        (true, false) => 0,
        (false, true) => 1,
        _ => return Err(EditError::Preservation),
    };
    let middle = partitions[target].len() / 2;
    if middle == 0 || middle == partitions[target].len() {
        return Err(EditError::Preservation);
    }
    let (lower, upper) = partitions[target].split_at(middle);
    let new_left =
        u32::try_from(proof.bytes.len() / PAGE_BYTES).map_err(|_| InspectError::StructureLimit)?;
    let retained_number = source.leaves[target].number;
    let (previous, next) = if target == 0 {
        (0, left_number)
    } else {
        (left_number, right_number)
    };

    // Every capacity and wire decision precedes map replay or page allocation.
    let fresh = fresh(definition, previous, next, lower, reader)?;
    let (mut retained, _) =
        render(raw(image, retained_number)?, upper, reader)?.ok_or(EditError::Preservation)?;
    retained[PREVIOUS..PREVIOUS + CHILD_BYTES].copy_from_slice(&new_left.to_le_bytes());
    let peer = 1 - target;
    let (mut peer_page, _) = rendered
        .get_mut(peer)
        .and_then(Option::take)
        .ok_or(EditError::Preservation)?;
    if target == 1 {
        peer_page[NEXT..NEXT + CHILD_BYTES].copy_from_slice(&new_left.to_le_bytes());
    }
    let maximum = lower.last().ok_or(EditError::Preservation)?;
    let bounds = if target == 0 {
        [(*maximum, new_left), (source.bound, left_number)]
    } else {
        [(source.bound, left_number), (*maximum, new_left)]
    };
    let root_page = branch(root, &bounds, reader)?;
    let pages = [source.root, left_number, right_number];
    for number in pages {
        let range = page_range(number)?;
        reader.work(PAGE_BYTES)?;
        if proof.bytes.get(range.clone()) != image.as_bytes().get(range) {
            return Err(EditError::Preservation);
        }
    }

    let mut allocation = Allocation::capture(image, index, definition, proof, reader)?;
    let actual_number = allocation.reserve(proof, reader)?;
    if actual_number != new_left {
        return Err(EditError::Preservation);
    }
    let (promoted, rebased) = allocation.finish(proof, reader)?;
    reader.work(4 * PAGE_BYTES)?;
    proof.bytes[page_range(new_left)?].copy_from_slice(&fresh);
    proof.bytes[page_range(source.root)?].copy_from_slice(&root_page);
    proof.bytes[page_range(retained_number)?].copy_from_slice(&retained);
    proof.bytes[page_range(source.leaves[peer].number)?].copy_from_slice(&peer_page);

    Ok(Growth {
        new_left,
        promoted,
        pages,
        begin: new_left,
        physical: index.data_number(),
        usage: index.usage_map(),
        rebased,
    })
}

impl Growth {
    pub(in crate::table::mutation::insert::audit::primary) fn compare(
        &self,
        actual: &PrimaryInsertion,
        proof: &ImageProof<'_>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        let extra = usize::from(self.promoted.is_some());
        let end = self
            .begin
            .checked_add(1 + extra as u32)
            .ok_or(EditError::Preservation)?;
        if self.new_left != self.begin
            || proof.bytes.len() / PAGE_BYTES != end as usize
            || actual.added.len() != 1 + extra
            || actual.promotions.len() != extra
        {
            return Err(EditError::Preservation);
        }

        let mut position = 0;
        for number in self.pages {
            let range = page_range(number)?;
            let before = proof
                .original
                .get(range.clone())
                .ok_or(EditError::Preservation)?;
            let after = proof
                .bytes
                .get(range.clone())
                .ok_or(EditError::Preservation)?;
            reader.work(PAGE_BYTES)?;
            if before == after {
                continue;
            }
            let patch = actual
                .patches
                .get(position)
                .ok_or(EditError::Preservation)?;
            if patch.offset != range.start || patch.before != before || patch.after != after {
                return Err(EditError::Preservation);
            }
            position += 1;
        }
        if position != actual.patches.len() {
            return Err(EditError::Preservation);
        }
        for (offset, page) in actual.added.iter().enumerate() {
            let number = self
                .begin
                .checked_add(offset as u32)
                .ok_or(EditError::Preservation)?;
            reader.work(PAGE_BYTES)?;
            if page.number != number || page.bytes != proof.bytes[page_range(number)?] {
                return Err(EditError::Preservation);
            }
        }
        if let Some(promotion) = &self.promoted {
            if promotion.number != self.begin.checked_add(1).ok_or(EditError::Preservation)? {
                return Err(EditError::Preservation);
            }
            let actual = &actual.promotions[0];
            reader.work(PAGE_BYTES + 2 * promotion.before.len())?;
            if actual.usage != self.usage
                || actual.number != promotion.number
                || actual.offset != promotion.offset
                || actual.before != promotion.before
                || actual.after != promotion.after
                || actual.bitmap != promotion.bitmap
            {
                return Err(EditError::Preservation);
            }
        }
        match self.rebased {
            Some(base) => {
                if actual.rebases.len() != 1 {
                    return Err(EditError::Preservation);
                }
                let actual = &actual.rebases[0];
                if actual.usage != self.usage
                    || actual.physical != self.physical
                    || actual.base != base
                {
                    return Err(EditError::Preservation);
                }
            }
            None if !actual.rebases.is_empty() => return Err(EditError::Preservation),
            None => {}
        }

        Ok(())
    }
}

fn fresh(
    definition: u32,
    previous: u32,
    next: u32,
    entries: &[[u8; WIRE_BYTES]],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<u8>, EditError> {
    let mut seed = copied(&[0; PAGE_BYTES], reader)?;
    seed[..2].copy_from_slice(&[LEAF, INDEX_VERSION]);
    seed[OWNER..OWNER + CHILD_BYTES].copy_from_slice(&definition.to_le_bytes());
    seed[PREVIOUS..PREVIOUS + CHILD_BYTES].copy_from_slice(&previous.to_le_bytes());
    seed[NEXT..NEXT + CHILD_BYTES].copy_from_slice(&next.to_le_bytes());
    render(&seed, entries, reader)?
        .map(|(page, _)| page)
        .ok_or(EditError::Preservation)
}

fn branch(
    original: &[u8],
    bounds: &[([u8; WIRE_BYTES], u32); 2],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<u8>, EditError> {
    let mut wires = [[0; BOUND_BYTES]; 2];
    for (wire, (maximum, child)) in wires.iter_mut().zip(bounds) {
        reader.work(BOUND_BYTES)?;
        if maximum[0] != KEY_MARKER {
            return Err(EditError::Preservation);
        }
        wire[..WIRE_BYTES].copy_from_slice(maximum);
        wire[WIRE_BYTES..].copy_from_slice(&child.to_be_bytes());
    }
    if wires[0][..WIRE_BYTES] >= wires[1][..WIRE_BYTES] {
        return Err(EditError::Preservation);
    }
    reader.work(BOUND_BYTES)?;
    let common = wires[0]
        .iter()
        .zip(&wires[1])
        .take_while(|(left, right)| left == right)
        .count();
    let seed = usize::from(IndexHeader::read(original)?.prefix.get());
    let mut prefix = seed.min(common);
    let total = BOUND_BYTES * wires.len();
    if total - prefix > CAPACITY {
        prefix = common;
    }
    let active = total - prefix;
    if active > CAPACITY {
        return Err(EditError::Preservation);
    }

    let mut after = copied(original, reader)?;
    reader.work(ENTRY_START - MASK_START + active)?;
    after[MASK_START..ENTRY_START].fill(0);
    after[FREE..FREE + size_of::<u16>()]
        .copy_from_slice(&((CAPACITY - active) as u16).to_le_bytes());
    after[PREFIX..PREFIX + size_of::<u16>()].copy_from_slice(&(prefix as u16).to_le_bytes());
    let mut end = 0;
    for (position, wire) in wires.iter().enumerate() {
        let suffix = &wire[if position == 0 { 0 } else { prefix }..];
        after[ENTRY_START + end..ENTRY_START + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        after[MASK_START + end / BITS] |= 1 << (end % BITS);
    }
    if end != active {
        return Err(EditError::Preservation);
    }

    Ok(after)
}
