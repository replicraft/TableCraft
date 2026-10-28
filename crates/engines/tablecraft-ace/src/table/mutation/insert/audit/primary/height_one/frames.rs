//! Audit-owned capacity and final complete-wire rendering.
use super::super::{INDEX_VERSION, KEY_MARKER, WIRE_BYTES, render, value};
use super::{copied, reserve};
use crate::{
    EditError, InspectError,
    format::{page::PAGE_BYTES, reader::Reader},
    table::indexes::{
        header::IndexHeader,
        page::{ENTRY_START, LEAF},
    },
};
use std::{io::Cursor, mem::offset_of};

const BOUND_BYTES: usize = WIRE_BYTES + size_of::<u32>();
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

pub(super) fn measure<const N: usize>(
    entries: &[[u8; N]],
    seed: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Layout, EditError> {
    let mut common = entries.first().map_or(0, |_| N);
    if let Some(first) = entries.first() {
        for entry in &entries[1..] {
            reader.work(common + 1)?;
            common = first[..common]
                .iter()
                .zip(entry)
                .take_while(|(left, right)| left == right)
                .count();
        }
    }
    measured(entries.len(), N, common, seed)
}

fn measured(count: usize, width: usize, common: usize, seed: usize) -> Result<Layout, EditError> {
    let total = count
        .checked_mul(width)
        .ok_or(InspectError::StructureLimit)?;
    let saved = count.saturating_sub(1);
    let mut prefix = seed.min(common);
    if total - prefix * saved > CAPACITY {
        prefix = common;
    }
    Ok(Layout {
        prefix,
        active: total - prefix * saved,
    })
}

pub(super) fn parent(
    maxima: &[[u8; WIRE_BYTES]],
    seed: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Layout, EditError> {
    for (position, entry) in maxima.iter().enumerate() {
        reader.tick()?;
        if entry[0] != KEY_MARKER || (position > 0 && value(&maxima[position - 1]) >= value(entry))
        {
            return Err(EditError::Preservation);
        }
    }
    let common = if maxima.len() == 1 {
        BOUND_BYTES
    } else {
        let mut common = maxima.first().map_or(0, |_| WIRE_BYTES);
        if let Some(first) = maxima.first() {
            for maximum in &maxima[1..] {
                reader.work(common + 1)?;
                common = first[..common]
                    .iter()
                    .zip(maximum)
                    .take_while(|(left, right)| left == right)
                    .count();
            }
        }
        common
    };
    // Distinct Int32 maxima differ within the known key bytes, before ROW/child.
    // No dummy child identity is needed to preflight the full 13-byte entries.
    measured(maxima.len(), BOUND_BYTES, common, seed)
}

pub(super) fn leaf(
    original: Option<&[u8]>,
    definition: u32,
    links: (u32, u32),
    entries: &[[u8; WIRE_BYTES]],
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

pub(super) fn root(
    original: &[u8],
    maxima: &[[u8; WIRE_BYTES]],
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
        let mut wire = [0; BOUND_BYTES];
        wire[..WIRE_BYTES].copy_from_slice(maximum);
        wire[WIRE_BYTES..].copy_from_slice(&child.to_be_bytes());
        wires.push(wire);
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
