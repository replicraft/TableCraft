//! Native directory framing and owner checks retain all historical record bytes.
use super::super::refused;
use crate::{
    EditError, InspectError,
    format::{
        bytes::{u16_at, u32_at},
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{Reader, row_count, row_range},
        row::{
            DELETED_MASK, OFFSET_MASK, RELOCATED_MASK, reference_page, reference_slot, slot_offset,
        },
    },
    table::indexes::{
        header::IndexHeader,
        page::{ENTRY_START, LEAF, NODE, PAGE_VERSION},
    },
};
use std::io::Cursor;

pub(super) fn framing(
    page: &[u8; PAGE_BYTES],
    owner: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<usize, EditError> {
    reader.tick()?;
    if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
        || u32_at(page, OWNER_OFFSET)? != owner
        || page[8..12] != [0; 4]
    {
        return Err(refused());
    }
    let count = row_count(page)?;
    // Packed row references have an eight-bit physical slot, not a consumer quota.
    if count > usize::from(u8::MAX) + 1 {
        return Err(refused());
    }
    let mut end = PAGE_BYTES;
    for slot in 0..count {
        reader.tick()?;
        let (word, start, previous) = row_range(page, slot)?;
        if word & !(OFFSET_MASK | DELETED_MASK | RELOCATED_MASK) != 0 {
            return Err(refused());
        }
        if start < slot_offset(count) || start >= previous || previous != end {
            return Err(refused());
        }
        end = start;
    }
    if usize::from(u16_at(page, 2)?) != end.checked_sub(slot_offset(count)).ok_or_else(refused)? {
        return Err(refused());
    }
    Ok(count)
}

pub(super) fn ordinary(
    number: u32,
    owner: u32,
    owned: &[u32],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<u32, EditError> {
    let page = reader.page(number)?;
    let count = framing(&page, owner, reader)?;
    let mut live = 0_u32;
    for slot in 0..count {
        reader.tick()?;
        let (word, start, end) = row_range(&page, slot)?;
        if word & DELETED_MASK != 0 {
            // DROP tombstones a logical root without erasing its original body.
            // A retained four-byte forwarding body still names actual old storage;
            // direct retired records remain opaque and are never decoded as pointers.
            if word & RELOCATED_MASK != 0 && end - start == size_of::<u32>() {
                let target = u32_at(&page[start..end], 0)?;
                reader.work(owned.len())?;
                if !owned.contains(&reference_page(target)) {
                    return Err(refused());
                }
                reader.row(reference_page(target), reference_slot(target), Some(owner))?;
            }
            continue;
        }
        let slot = u8::try_from(slot).map_err(|_| refused())?;
        reader.read_location_observed(number, slot, owner, &page, &mut |reference, budget| {
            budget.charge(tablecraft_core::limits::Resource::Steps, owned.len() as u64)?;
            if !owned.contains(&crate::format::row::reference_page(reference)) {
                return Err(InspectError::Unsupported);
            }
            Ok(())
        })?;
        live = live.checked_add(1).ok_or_else(refused)?;
    }
    Ok(live)
}

pub(super) fn index(
    number: u32,
    owner: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let page = reader.page(number)?;
    let header = IndexHeader::read(&page)?;
    if !matches!(header.kind, LEAF | NODE)
        || header.version != PAGE_VERSION
        || header.owner.get() != owner
    {
        return Err(refused());
    }
    for target in [header.previous.get(), header.next.get(), header.tail.get()] {
        reader.tick()?;
        if target >= reader.header.page_count() && target != 0 {
            return Err(refused());
        }
    }
    Ok(())
}

/// A retired table has no live membership; retain stale key tails byte for byte.
fn retained_empty(header: &IndexHeader, page: &[u8; PAGE_BYTES]) -> bool {
    header.kind == LEAF
        && header.previous.get() == 0
        && header.next.get() == 0
        && header.tail.get() == 0
        && header.prefix.get() == 0
        && header.qualified_for_update(0)
        && usize::from(header.free.get()) == PAGE_BYTES - ENTRY_START
        && page[size_of::<IndexHeader>()..ENTRY_START]
            .iter()
            .all(|byte| *byte == 0)
}

pub(super) fn retired_index(
    number: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let page = reader.page(number)?;
    reader.work(ENTRY_START)?;
    if !retained_empty(IndexHeader::read(&page)?, &page) {
        return Err(refused());
    }
    Ok(())
}
