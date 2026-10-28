//! Audit-owned full-wire capacity and original/fresh page framing.
use super::{Wire, copied, reference};
use crate::{
    EditError,
    format::{page::PAGE_BYTES, reader::Reader},
    table::indexes::{
        header::{IndexHeader, LEVEL_OFFSET},
        page::{ENTRY_START, LEAF, NODE},
    },
};
use std::{io::Cursor, mem::offset_of};

const MASK_START: usize = size_of::<IndexHeader>();
const FREE: usize = offset_of!(IndexHeader, free);
const PREFIX: usize = offset_of!(IndexHeader, prefix);
const OWNER: usize = offset_of!(IndexHeader, owner);
const PREVIOUS: usize = offset_of!(IndexHeader, previous);
const NEXT: usize = offset_of!(IndexHeader, next);
const TAIL: usize = offset_of!(IndexHeader, tail);
const KIND: usize = offset_of!(IndexHeader, kind);
const VERSION: usize = offset_of!(IndexHeader, version);
const CAPACITY: usize = PAGE_BYTES - ENTRY_START;
const BITS: usize = u8::BITS as usize;

pub(super) struct Layout {
    pub prefix: usize,
    pub active: usize,
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
    let (prefix, active) = super::super::layout(entries, seed, reader)?;
    Ok(Layout { prefix, active })
}

pub(super) fn membership(
    original: &[Wire],
    entries: &[Wire],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    if entries.len() <= original.len() {
        return Err(EditError::Preservation);
    }
    for pair in entries.windows(2) {
        reader.tick()?;
        if pair[0] >= pair[1] {
            return Err(EditError::Preservation);
        }
    }
    if entries
        .last()
        .is_none_or(|entry| entry.len() < 1 + size_of::<u32>())
    {
        return Err(EditError::Preservation);
    }
    for entry in original {
        reader.work(entries.len())?;
        if entries.binary_search(entry).is_err() {
            return Err(EditError::Preservation);
        }
    }
    for (position, entry) in entries.iter().enumerate() {
        reader.work(position + 1)?;
        if entries[..position]
            .iter()
            .any(|prior| reference(prior) == reference(entry))
        {
            return Err(EditError::Preservation);
        }
    }

    Ok(())
}

pub(super) fn leaf(
    owner: u32,
    previous: u32,
    next: u32,
    entries: &[Wire],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<u8>, EditError> {
    let layout = measure(entries, 0, reader)?;
    if entries.is_empty() || !layout.fits() {
        return Err(EditError::Preservation);
    }
    let mut after = copied(&[0; PAGE_BYTES], reader)?;
    after[KIND] = LEAF;
    after[VERSION] = 1;
    after[OWNER..OWNER + size_of::<u32>()].copy_from_slice(&owner.to_le_bytes());
    after[PREVIOUS..PREVIOUS + size_of::<u32>()].copy_from_slice(&previous.to_le_bytes());
    after[NEXT..NEXT + size_of::<u32>()].copy_from_slice(&next.to_le_bytes());
    after[FREE..FREE + size_of::<u16>()]
        .copy_from_slice(&((CAPACITY - layout.active) as u16).to_le_bytes());
    after[PREFIX..PREFIX + size_of::<u16>()].copy_from_slice(&(layout.prefix as u16).to_le_bytes());
    let mut end = 0;
    for (position, entry) in entries.iter().enumerate() {
        reader.work(entry.len() + 1)?;
        let shared = if position == 0 { 0 } else { layout.prefix };
        let next = end + entry.len() - shared;
        after[ENTRY_START + end..ENTRY_START + next].copy_from_slice(&entry[shared..]);
        after[MASK_START + next / BITS] |= 1 << (next % BITS);
        end = next;
    }
    if end != layout.active {
        return Err(EditError::Preservation);
    }

    Ok(after)
}

pub(super) fn root(
    original: &[u8],
    left: u32,
    right: u32,
    maximum: &Wire,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<u8>, EditError> {
    let mut after = copied(original, reader)?;
    let active = maximum.len() + size_of::<u32>();
    reader.work(ENTRY_START - MASK_START + active)?;
    after[KIND] = NODE;
    after[LEVEL_OFFSET] = 1;
    after[TAIL..TAIL + size_of::<u32>()].copy_from_slice(&right.to_le_bytes());
    after[PREFIX..PREFIX + size_of::<u16>()].fill(0);
    after[FREE..FREE + size_of::<u16>()]
        .copy_from_slice(&((CAPACITY - active) as u16).to_le_bytes());
    after[MASK_START..ENTRY_START].fill(0);
    after[ENTRY_START..ENTRY_START + maximum.len()].copy_from_slice(maximum);
    after[ENTRY_START + maximum.len()..ENTRY_START + active].copy_from_slice(&left.to_be_bytes());
    after[MASK_START + active / BITS] |= 1 << (active % BITS);

    Ok(after)
}
