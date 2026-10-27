//! Raw framing and opaque-byte proof independent of page construction.
use super::super::super::{encoding, profile::SingleProfile};
use super::super::Certificate;
use super::AUDIT_CHUNK_BYTES;
use crate::{
    EditError, InspectError,
    format::page::PAGE_BYTES,
    header::check,
    table::indexes::{
        header::{IndexHeader, LEVEL_OFFSET},
        page::{CHILD_BYTES, ENTRY_START, Entry, KeyRef, LEAF, NODE, ROW_BYTES},
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const MASK_START: usize = size_of::<IndexHeader>();
const MASK_BYTES: usize = ENTRY_START - MASK_START;
const FREE_OFFSET: usize = std::mem::offset_of!(IndexHeader, free);
const PREFIX_OFFSET: usize = std::mem::offset_of!(IndexHeader, prefix);
const TAIL_OFFSET: usize = std::mem::offset_of!(IndexHeader, tail);

pub(super) fn bitmap(
    raw: &[u8],
    entries: &[KeyRef],
    child: u32,
    profile: &SingleProfile,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, EditError> {
    let header = IndexHeader::read(raw)?;
    let prefix = usize::from(header.prefix.get());
    let mut expected = [0_u8; MASK_BYTES];
    let mut end = 0_usize;
    for (position, value) in entries.iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, encoding::MAX_ENTRY_BYTES as u64)
            .map_err(InspectError::from)?;
        let wire = encoding::entry::<_, { encoding::MAX_ENTRY_BYTES }>(
            &Entry {
                value: *value,
                child,
            },
            profile,
        )?;
        let length = wire
            .length
            .checked_sub(if position == 0 { 0 } else { prefix })
            .ok_or(EditError::Preservation)?;
        end = end
            .checked_add(length)
            .filter(|&end| end <= PAGE_BYTES - ENTRY_START)
            .ok_or(EditError::Preservation)?;
        expected[end / u8::BITS as usize] |= 1 << (end % u8::BITS as usize);
    }
    budget
        .charge(Resource::Steps, MASK_BYTES as u64)
        .map_err(InspectError::from)?;
    if raw[MASK_START..ENTRY_START] != expected
        || usize::from(header.free.get()) != PAGE_BYTES - ENTRY_START - end
    {
        return Err(EditError::Preservation);
    }
    Ok(ENTRY_START + end)
}

pub(super) fn leaf(
    raw: &[u8],
    owner: u32,
    links: (u32, u32),
    expected: &[KeyRef],
    profile: &SingleProfile,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let header = IndexHeader::read(raw)?;
    if header.kind != LEAF
        || header.version != 1
        || header.owner.get() != owner
        || !header.qualified_for_update(0)
        || header.previous.get() != links.0
        || header.next.get() != links.1
        || header.tail.get() != 0
        || expected.is_empty()
    {
        return Err(EditError::Preservation);
    }
    let end = bitmap(raw, expected, 0, profile, budget, cancelled)?;
    budget
        .charge(Resource::Steps, (PAGE_BYTES - end) as u64)
        .map_err(InspectError::from)?;
    for chunk in raw[end..].chunks(AUDIT_CHUNK_BYTES) {
        check(budget, cancelled)?;
        if chunk.iter().any(|byte| *byte != 0) {
            return Err(EditError::Preservation);
        }
    }
    Ok(())
}

pub(super) fn root(
    before: &[u8],
    after: &[u8],
    fact: &Certificate,
    maximum: KeyRef,
    profile: &SingleProfile,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let left = fact.left;
    let right = fact.right;
    let original = IndexHeader::read(before)?;
    let header = IndexHeader::read(after)?;
    if original.kind != LEAF
        || original.previous.get() != 0
        || original.next.get() != 0
        || !original.qualified_for_update(0)
        || header.kind != NODE
        || !header.qualified_for_update(1)
        || header.prefix.get() != 0
        || header.previous.get() != 0
        || header.next.get() != 0
        || header.tail.get() != right
    {
        return Err(EditError::Preservation);
    }
    let end = bitmap(after, &[maximum], left, profile, budget, cancelled)?;
    budget
        .charge(Resource::Steps, PAGE_BYTES as u64)
        .map_err(InspectError::from)?;
    for position in 0..PAGE_BYTES {
        if position.is_multiple_of(AUDIT_CHUNK_BYTES) {
            check(budget, cancelled)?;
        }
        let allowed = position == 0
            || (FREE_OFFSET..FREE_OFFSET + size_of::<u16>()).contains(&position)
            || (TAIL_OFFSET..TAIL_OFFSET + size_of::<u32>()).contains(&position)
            || (PREFIX_OFFSET..PREFIX_OFFSET + size_of::<u16>()).contains(&position)
            || position == LEVEL_OFFSET
            || (MASK_START..end).contains(&position);
        if !allowed && before[position] != after[position] {
            return Err(EditError::Preservation);
        }
    }
    // Node entries carry both the complete key/row maximum and a child pointer.
    if end <= ENTRY_START + ROW_BYTES + CHILD_BYTES {
        return Err(EditError::Preservation);
    }
    Ok(())
}
