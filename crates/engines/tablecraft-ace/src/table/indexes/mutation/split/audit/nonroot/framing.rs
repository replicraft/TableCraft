//! Independently measure active framing and protect old/fresh unused bytes.
use super::super::super::super::{encoding, profile::SingleProfile};
use super::super::AUDIT_CHUNK_BYTES;
use crate::{
    EditError, InspectError,
    format::{page::PAGE_BYTES, reader::Reader},
    header::check,
    table::indexes::{
        header::{IndexHeader, LEVEL_OFFSET},
        page::{ENTRY_START, Entry, LEAF, NODE},
    },
};
use std::io::Cursor;
use tablecraft_core::limits::Resource;

const KIND_OFFSET: usize = std::mem::offset_of!(IndexHeader, kind);
const FREE_OFFSET: usize = std::mem::offset_of!(IndexHeader, free);
const PREFIX_OFFSET: usize = std::mem::offset_of!(IndexHeader, prefix);
const PREVIOUS_OFFSET: usize = std::mem::offset_of!(IndexHeader, previous);
const NEXT_OFFSET: usize = std::mem::offset_of!(IndexHeader, next);
const TAIL_OFFSET: usize = std::mem::offset_of!(IndexHeader, tail);
const OWNER_OFFSET: usize = std::mem::offset_of!(IndexHeader, owner);
const VERSION_OFFSET: usize = std::mem::offset_of!(IndexHeader, version);
const MASK_START: usize = size_of::<IndexHeader>();
const MASK_BYTES: usize = ENTRY_START - MASK_START;

#[derive(Clone, Copy)]
pub(super) enum Source<'a> {
    Existing(&'a [u8]),
    GrownRoot(&'a [u8]),
    Fresh,
}

pub(super) struct Spec<'a> {
    pub source: Source<'a>,
    pub after: &'a [u8],
    pub entries: &'a [Entry],
    pub profile: &'a SingleProfile,
    pub owner: u32,
    pub height: u8,
    pub tail: u32,
}

pub(super) fn prove(
    spec: &Spec<'_>,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let header = IndexHeader::read(spec.after)?;
    if header.kind != if spec.height == 0 { LEAF } else { NODE }
        || header.version != 1
        || header.owner.get() != spec.owner
        || !header.qualified_for_update(spec.height)
        || header.tail.get() != spec.tail
    {
        return Err(EditError::Preservation);
    }
    let (before, seed, fresh, grown) = match spec.source {
        Source::Existing(before) => {
            if !IndexHeader::read(before)?.qualified_for_update(spec.height) {
                return Err(EditError::Preservation);
            }
            (
                Some(before),
                usize::from(IndexHeader::read(before)?.prefix.get()),
                false,
                false,
            )
        }
        Source::GrownRoot(before) => {
            let old = IndexHeader::read(before)?;
            let leaf_root = old.kind == LEAF && before[LEVEL_OFFSET] == 0;
            if (!leaf_root && old.kind != NODE)
                || before[LEVEL_OFFSET].checked_add(1) != Some(spec.height)
                || old.previous.get() != 0
                || old.next.get() != 0
                || (leaf_root && spec.tail == 0)
                || (!leaf_root && spec.tail != 0)
            {
                return Err(EditError::Preservation);
            }
            (Some(before), 0, false, true)
        }
        Source::Fresh => (None, 0, true, false),
    };
    let end = layout(spec.entries, seed, spec.after, spec.profile, actual)?;
    actual.work(PAGE_BYTES)?;
    for position in 0..PAGE_BYTES {
        if position.is_multiple_of(AUDIT_CHUNK_BYTES) {
            check(actual.budget, actual.cancelled)?;
        }
        let allowed = (FREE_OFFSET..FREE_OFFSET + size_of::<u16>()).contains(&position)
            || (PREFIX_OFFSET..PREFIX_OFFSET + size_of::<u16>()).contains(&position)
            || (PREVIOUS_OFFSET..NEXT_OFFSET + size_of::<u32>()).contains(&position)
            || (MASK_START..end).contains(&position)
            || (fresh || grown)
                && ((TAIL_OFFSET..TAIL_OFFSET + size_of::<u32>()).contains(&position)
                    || position == LEVEL_OFFSET
                    || (grown && position == KIND_OFFSET))
            || fresh
                && (position == KIND_OFFSET
                    || position == VERSION_OFFSET
                    || (OWNER_OFFSET..OWNER_OFFSET + size_of::<u32>()).contains(&position));
        let original = before.map_or(0, |bytes| bytes[position]);
        if !allowed && spec.after[position] != original {
            return Err(EditError::Preservation);
        }
    }
    Ok(())
}

struct Measurement {
    prefix: usize,
    bytes: usize,
}

pub(super) fn fits(
    entries: &[Entry],
    seed: usize,
    profile: &SingleProfile,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<bool, EditError> {
    Ok(measure(entries, seed, profile, actual)?.bytes <= PAGE_BYTES - ENTRY_START)
}

fn measure(
    entries: &[Entry],
    original_prefix: usize,
    profile: &SingleProfile,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Measurement, EditError> {
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
    let retained = total
        .checked_sub(prefix.checked_mul(saved).ok_or(EditError::Preservation)?)
        .ok_or(EditError::Preservation)?;
    if retained > PAGE_BYTES - ENTRY_START {
        prefix = common;
    }
    let bytes = total
        .checked_sub(prefix.checked_mul(saved).ok_or(EditError::Preservation)?)
        .ok_or(EditError::Preservation)?;
    Ok(Measurement { prefix, bytes })
}

pub(super) fn layout(
    entries: &[Entry],
    original_prefix: usize,
    raw: &[u8],
    profile: &SingleProfile,
    actual: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<usize, EditError> {
    let measured = measure(entries, original_prefix, profile, actual)?;
    let prefix = measured.prefix;
    if measured.bytes > PAGE_BYTES - ENTRY_START {
        return Err(EditError::Preservation);
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
    if end != measured.bytes
        || raw[MASK_START..ENTRY_START] != expected_bitmap
        || usize::from(header.free.get()) != PAGE_BYTES - ENTRY_START - end
    {
        return Err(EditError::Preservation);
    }
    Ok(ENTRY_START + end)
}

#[cfg(test)]
mod tests;
