//! Audit-owned capacity and final complete-wire rendering.
use super::super::{INDEX_VERSION, Wire, layout, render};
use super::{copied, reserve};
use crate::{
    EditError, InspectError,
    format::{page::PAGE_BYTES, reader::Reader},
    table::indexes::{
        header::{IndexHeader, LEVEL_OFFSET},
        page::{ENTRY_START, LEAF, NODE},
    },
};
use std::{io::Cursor, mem::offset_of};

const MASK: usize = size_of::<IndexHeader>();
const BITS: usize = u8::BITS as usize;
const FREE: usize = offset_of!(IndexHeader, free);
const PREFIX: usize = offset_of!(IndexHeader, prefix);
const OWNER: usize = offset_of!(IndexHeader, owner);
const PREVIOUS: usize = offset_of!(IndexHeader, previous);
const NEXT: usize = offset_of!(IndexHeader, next);
const TAIL: usize = offset_of!(IndexHeader, tail);
const CAPACITY: usize = PAGE_BYTES - ENTRY_START;

pub(super) struct Layout {
    prefix: usize,
    active: usize,
}

impl Layout {
    pub(super) fn fits(&self) -> bool {
        self.active <= CAPACITY
    }
}

pub(super) fn measure(
    entries: &[Wire],
    seed: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Layout, EditError> {
    let (prefix, active) = layout(entries, seed, reader)?;
    Ok(Layout { prefix, active })
}

pub(super) fn parent(
    maxima: &[Wire],
    seed: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Layout, EditError> {
    let mut total = 0_usize;
    let mut common = maxima.first().map_or(0, |entry| entry.len());
    for (position, entry) in maxima.iter().enumerate() {
        reader.work(entry.len() + 1)?;
        if position > 0 && maxima[position - 1] >= *entry {
            return Err(EditError::Preservation);
        }
        total = total
            .checked_add(entry.len() + size_of::<u32>())
            .ok_or(InspectError::StructureLimit)?;
        if let Some(first) = maxima.first() {
            common = first[..common]
                .iter()
                .zip(entry)
                .take_while(|(left, right)| left == right)
                .count();
        }
    }
    if maxima.len() == 1 {
        common += size_of::<u32>();
    }
    // Distinct full key/ROW maxima differ before their child pointer. Therefore
    // sizing uses actual maximum lengths without inventing child page numbers.
    let saved = maxima.len().saturating_sub(1);
    let mut prefix = seed.min(common);
    if total - saved * prefix > CAPACITY {
        prefix = common;
    }
    Ok(Layout {
        prefix,
        active: total - saved * prefix,
    })
}

pub(super) fn leaf(
    original: Option<&[u8]>,
    definition: u32,
    links: (u32, u32),
    entries: &[Wire],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<u8>, EditError> {
    let mut seed = match original {
        Some(bytes) => copied(bytes, reader)?,
        None => {
            let mut bytes = copied(&[0; PAGE_BYTES], reader)?;
            bytes[..2].copy_from_slice(&[LEAF, INDEX_VERSION]);
            bytes[OWNER..OWNER + size_of::<u32>()].copy_from_slice(&definition.to_le_bytes());
            bytes
        }
    };
    seed[PREVIOUS..PREVIOUS + size_of::<u32>()].copy_from_slice(&links.0.to_le_bytes());
    seed[NEXT..NEXT + size_of::<u32>()].copy_from_slice(&links.1.to_le_bytes());
    render(&seed, entries, reader)?
        .map(|(page, _)| page)
        .ok_or(EditError::Preservation)
}

pub(super) fn branch(
    original: &[u8],
    maxima: &[Wire],
    children: &[u32],
    tail: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<u8>, EditError> {
    if maxima.len() != children.len() {
        return Err(EditError::Preservation);
    }
    let seed = usize::from(IndexHeader::read(original)?.prefix.get());
    let predicted = parent(maxima, seed, reader)?;
    let mut wires = Vec::new();
    reserve(&mut wires, maxima.len(), reader)?;
    for (maximum, child) in maxima.iter().zip(children) {
        reader.tick()?;
        reader.work(maximum.len() + size_of::<u32>())?;
        wires.push(maximum.child_paid(*child, reader)?);
    }
    let actual = measure(&wires, seed, reader)?;
    if !actual.fits() || actual.prefix != predicted.prefix || actual.active != predicted.active {
        return Err(EditError::Preservation);
    }

    let mut after = copied(original, reader)?;
    reader.work(ENTRY_START - MASK + actual.active)?;
    after[MASK..ENTRY_START].fill(0);
    after[FREE..FREE + size_of::<u16>()]
        .copy_from_slice(&((CAPACITY - actual.active) as u16).to_le_bytes());
    after[PREFIX..PREFIX + size_of::<u16>()].copy_from_slice(&(actual.prefix as u16).to_le_bytes());
    after[TAIL..TAIL + size_of::<u32>()].copy_from_slice(&tail.to_le_bytes());
    let mut end = 0;
    for (position, wire) in wires.iter().enumerate() {
        reader.tick()?;
        let suffix = &wire[if position == 0 { 0 } else { actual.prefix }..];
        after[ENTRY_START + end..ENTRY_START + end + suffix.len()].copy_from_slice(suffix);
        end += suffix.len();
        after[MASK + end / BITS] |= 1 << (end % BITS);
    }
    if end != actual.active {
        return Err(EditError::Preservation);
    }

    Ok(after)
}

pub(super) fn seed(
    original: Option<&[u8]>,
    definition: u32,
    height: u8,
    links: (u32, u32),
    reset_prefix: bool,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<u8>, EditError> {
    let mut bytes = match original {
        Some(bytes) => copied(bytes, reader)?,
        None => copied(&[0; PAGE_BYTES], reader)?,
    };
    bytes[..2].copy_from_slice(&[if height == 0 { LEAF } else { NODE }, INDEX_VERSION]);
    bytes[OWNER..OWNER + size_of::<u32>()].copy_from_slice(&definition.to_le_bytes());
    bytes[PREVIOUS..PREVIOUS + size_of::<u32>()].copy_from_slice(&links.0.to_le_bytes());
    bytes[NEXT..NEXT + size_of::<u32>()].copy_from_slice(&links.1.to_le_bytes());
    bytes[LEVEL_OFFSET] = height;
    if reset_prefix {
        bytes[PREFIX..PREFIX + size_of::<u16>()].fill(0);
    }
    Ok(bytes)
}
