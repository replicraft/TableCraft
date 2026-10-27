//! Re-derive leaf insertion and ancestor bounds from original checked topology.
use super::super::super::{encoding, profile::SingleProfile, repack, tree::Tree};
use super::super::leaf_entries;
use super::{AUDIT_CHUNK_BYTES, framing, maps};
use crate::{
    EditError, InspectError, NativeImage,
    format::{page::PAGE_BYTES, reader::Reader},
    header::check,
    table::indexes::{
        header::IndexHeader,
        page::{self, ENTRY_START, Entry, KeyRef},
    },
};
use std::io::Cursor;
use tablecraft_core::limits::Resource;

const FREE_OFFSET: usize = std::mem::offset_of!(IndexHeader, free);
const PREFIX_OFFSET: usize = std::mem::offset_of!(IndexHeader, prefix);
const PREVIOUS_OFFSET: usize = std::mem::offset_of!(IndexHeader, previous);
const NEXT_OFFSET: usize = std::mem::offset_of!(IndexHeader, next);
const MASK_START: usize = size_of::<IndexHeader>();
const MASK_BYTES: usize = ENTRY_START - MASK_START;

pub(super) struct Operation<'a> {
    pub image: &'a NativeImage,
    pub tree: &'a Tree,
    pub keys: &'a [KeyRef],
    pub profile: &'a SingleProfile,
    pub owner: u32,
    pub usage: u32,
}

pub(super) struct Progress<'a> {
    pub next_eof: &'a mut u32,
    pub map_edits: &'a mut Vec<maps::Expected>,
}

struct Leaf<'a> {
    number: u32,
    entries: &'a [KeyRef],
    left: Option<u32>,
}

struct Bound {
    maximum: KeyRef,
    left: Option<(u32, KeyRef)>,
}

pub(super) fn prove(
    operation: &Operation<'_>,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
    progress: &mut Progress<'_>,
) -> Result<Vec<u32>, EditError> {
    let mut leaves = Vec::new();
    let mut remaining = operation.keys;
    partition(operation.tree, &mut remaining, &mut leaves, actual)?;
    if !remaining.is_empty() {
        return Err(EditError::Preservation);
    }
    let mut added = Vec::new();
    let mut source = operation.image.reader();
    for leaf in &mut leaves {
        actual.tick()?;
        let entries = leaf_entries(leaf.entries, actual.budget, actual.cancelled)?;
        if !repack::fits::<_, { encoding::MAX_ENTRY_BYTES }>(
            raw(operation.image, leaf.number)?,
            &entries,
            operation.profile,
            actual.budget,
            actual.cancelled,
        )? {
            if leaf.entries.len() < 2 {
                return Err(EditError::Preservation);
            }
            let number = *progress.next_eof;
            *progress.next_eof = number.checked_add(1).ok_or(EditError::Preservation)?;
            leaf.left = Some(number);
            actual
                .budget
                .charge_allocation(1, size_of::<u32>() as u64)
                .map_err(InspectError::from)?;
            added.try_reserve(1).map_err(|_| InspectError::Allocation)?;
            added.push(number);
            maps::add(
                &mut Reader {
                    source: &mut source,
                    header: operation.image.catalog().header(),
                    budget: actual.budget,
                    cancelled: actual.cancelled,
                    reads: 0,
                },
                operation.usage,
                [number],
                progress.map_edits,
            )?;
        }
    }
    verify(operation, operation.tree, &leaves, actual)?;
    Ok(added)
}

fn partition<'a>(
    tree: &Tree,
    remaining: &mut &'a [KeyRef],
    leaves: &mut Vec<Leaf<'a>>,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    actual.tick()?;
    if tree.is_leaf() {
        let (entries, rest) = remaining
            .split_at_checked(tree.count())
            .ok_or(EditError::Preservation)?;
        *remaining = rest;
        actual
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
            partition(child, remaining, leaves, actual)?;
        }
    }
    Ok(())
}

fn verify(
    operation: &Operation<'_>,
    tree: &Tree,
    leaves: &[Leaf<'_>],
    actual: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Bound, EditError> {
    actual.tick()?;
    let original = raw(operation.image, tree.number())?;
    if tree.is_leaf() {
        actual.work(leaves.len())?;
        let position = leaves
            .iter()
            .position(|leaf| leaf.number == tree.number())
            .ok_or(EditError::Preservation)?;
        let leaf = &leaves[position];
        let previous = position
            .checked_sub(1)
            .map(|prior| leaves[prior].number)
            .unwrap_or(0);
        let next = leaves
            .get(position + 1)
            .map(|leaf| leaf.left.unwrap_or(leaf.number))
            .unwrap_or(0);
        let mut expected = leaf.entries;
        let mut left = None;
        if let Some(number) = leaf.left {
            let middle = expected.len() / 2;
            let page = page::read_profile::<_, _, { encoding::MAX_ENTRY_BYTES }>(
                actual,
                number,
                operation.owner,
                operation.profile,
            )?;
            values(&page, &expected[..middle], actual)?;
            let bytes = actual.page(number)?;
            framing::leaf(
                &bytes,
                operation.owner,
                (previous, tree.number()),
                &expected[..middle],
                operation.profile,
                actual.budget,
                actual.cancelled,
            )?;
            // New pages start with a zero prefix; compression is required only
            // when the complete uncompressed entries would exceed capacity.
            let entries = leaf_entries(&expected[..middle], actual.budget, actual.cancelled)?;
            layout(&entries, 0, &bytes, operation.profile, actual)?;
            left = Some((number, expected[middle - 1]));
            expected = &expected[middle..];
        }
        let page = page::read_profile::<_, _, { encoding::MAX_ENTRY_BYTES }>(
            actual,
            tree.number(),
            operation.owner,
            operation.profile,
        )?;
        values(&page, expected, actual)?;
        let bytes = actual.page(tree.number())?;
        let links = (leaf.left.unwrap_or(previous), next);
        if (page.previous, page.next) != links || page.tail != 0 {
            return Err(EditError::Preservation);
        }
        let entries = leaf_entries(expected, actual.budget, actual.cancelled)?;
        retained(
            &Frame {
                before: original,
                after: &bytes,
                entries: &entries,
                profile: operation.profile,
                links: Some(links),
            },
            actual,
        )?;
        return Ok(Bound {
            maximum: *expected.last().ok_or(EditError::Preservation)?,
            left,
        });
    }
    let original_header = IndexHeader::read(original)?;
    let mut expected = Vec::new();
    let mut maximum = None;
    for (position, child) in tree.children().iter().enumerate() {
        let bound = verify(operation, child, leaves, actual)?;
        if let Some((number, value)) = bound.left {
            push(
                &mut expected,
                Entry {
                    value,
                    child: number,
                },
                actual,
            )?;
        }
        if position < tree.count() {
            push(
                &mut expected,
                Entry {
                    value: bound.maximum,
                    child: child.number(),
                },
                actual,
            )?;
        }
        maximum = Some(bound.maximum);
    }
    let page = page::read_profile::<_, _, { encoding::MAX_ENTRY_BYTES }>(
        actual,
        tree.number(),
        operation.owner,
        operation.profile,
    )?;
    if page.leaf || page.tail != original_header.tail.get() || page.entries.len() != expected.len()
    {
        return Err(EditError::Preservation);
    }
    for (entry, expected) in page.entries.iter().zip(&expected) {
        actual.tick()?;
        if entry.value != expected.value || entry.child != expected.child {
            return Err(EditError::Preservation);
        }
    }
    let bytes = actual.page(tree.number())?;
    retained(
        &Frame {
            before: original,
            after: &bytes,
            entries: &expected,
            profile: operation.profile,
            links: None,
        },
        actual,
    )?;
    Ok(Bound {
        maximum: maximum.ok_or(EditError::Preservation)?,
        left: None,
    })
}

fn values(
    page: &page::Page,
    expected: &[KeyRef],
    actual: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    if !page.leaf || page.entries.len() != expected.len() {
        return Err(EditError::Preservation);
    }
    for (entry, expected) in page.entries.iter().zip(expected) {
        actual.tick()?;
        if entry.child != 0 || entry.value != *expected {
            return Err(EditError::Preservation);
        }
    }
    Ok(())
}

struct Frame<'a> {
    before: &'a [u8],
    after: &'a [u8],
    entries: &'a [Entry],
    profile: &'a SingleProfile,
    links: Option<(u32, u32)>,
}

fn retained(frame: &Frame<'_>, actual: &mut Reader<'_, Cursor<&[u8]>>) -> Result<(), EditError> {
    let prefix = usize::from(IndexHeader::read(frame.before)?.prefix.get());
    let end = layout(frame.entries, prefix, frame.after, frame.profile, actual)?;
    actual.work(PAGE_BYTES)?;
    for position in 0..PAGE_BYTES {
        if position.is_multiple_of(AUDIT_CHUNK_BYTES) {
            check(actual.budget, actual.cancelled)?;
        }
        let allowed = (FREE_OFFSET..FREE_OFFSET + size_of::<u16>()).contains(&position)
            || (PREFIX_OFFSET..PREFIX_OFFSET + size_of::<u16>()).contains(&position)
            || (MASK_START..end).contains(&position)
            || frame.links.is_some()
                && (PREVIOUS_OFFSET..NEXT_OFFSET + size_of::<u32>()).contains(&position);
        if !allowed && frame.before[position] != frame.after[position] {
            return Err(EditError::Preservation);
        }
    }
    Ok(())
}

fn layout(
    entries: &[Entry],
    original_prefix: usize,
    raw: &[u8],
    profile: &SingleProfile,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<usize, EditError> {
    let mut total = 0_usize;
    let mut common = 0_usize;
    let mut first = [0; encoding::MAX_ENTRY_BYTES];
    for (position, entry) in entries.iter().enumerate() {
        actual.tick()?;
        actual.work(encoding::MAX_ENTRY_BYTES)?;
        let wire = encoding::entry::<_, { encoding::MAX_ENTRY_BYTES }>(entry, profile)?;
        total = total
            .checked_add(wire.length)
            .ok_or(EditError::Preservation)?;
        if position == 0 {
            first = wire.bytes;
            common = if entries.len() == 1 {
                wire.length
            } else {
                wire.key_length
            };
        } else {
            common = common.min(wire.key_length);
            common = first[..common]
                .iter()
                .zip(&wire.bytes[..common])
                .take_while(|(a, b)| a == b)
                .count();
        }
    }
    let saved = entries.len().saturating_sub(1);
    let mut prefix = original_prefix.min(common);
    if total - prefix * saved > PAGE_BYTES - ENTRY_START {
        prefix = common;
    }
    let header = IndexHeader::read(raw)?;
    if usize::from(header.prefix.get()) != prefix {
        return Err(EditError::Preservation);
    }
    let mut expected_bitmap = [0_u8; MASK_BYTES];
    let mut end = 0_usize;
    for (position, entry) in entries.iter().enumerate() {
        actual.tick()?;
        actual.work(encoding::MAX_ENTRY_BYTES)?;
        let wire = encoding::entry::<_, { encoding::MAX_ENTRY_BYTES }>(entry, profile)?;
        end = end
            .checked_add(
                wire.length
                    .checked_sub(if position == 0 { 0 } else { prefix })
                    .ok_or(EditError::Preservation)?,
            )
            .filter(|end| *end <= PAGE_BYTES - ENTRY_START)
            .ok_or(EditError::Preservation)?;
        expected_bitmap[end / u8::BITS as usize] |= 1 << (end % u8::BITS as usize);
    }
    actual
        .budget
        .charge(Resource::Steps, MASK_BYTES as u64)
        .map_err(InspectError::from)?;
    if raw[MASK_START..ENTRY_START] != expected_bitmap
        || usize::from(header.free.get()) != PAGE_BYTES - ENTRY_START - end
    {
        return Err(EditError::Preservation);
    }
    Ok(ENTRY_START + end)
}

fn push(
    entries: &mut Vec<Entry>,
    entry: Entry,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    actual
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
