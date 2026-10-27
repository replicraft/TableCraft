//! Replay inline row rewrites from source membership, without builder framing.
use super::{
    Byte, Expected, INLINE_TAG, PAGES_IN_BITMAP, POINTER_BYTES, PREFIX_BYTES, Promoted,
    QUALIFIED_ROW_BYTES, Rebased, ReplayPolicy, bit,
};
use crate::{
    EditError, InspectError, WriteBlocker,
    format::{
        allocation::{self, RepresentedMap},
        reader::Reader,
        row::{reference_page, reference_slot},
    },
};
use std::io::Cursor;

const INLINE_CAPACITY: u32 = 512;

pub(super) fn reserve(
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    usage: u32,
    number: u32,
    next_eof: &mut u32,
    expected: &mut Expected,
    global: &RepresentedMap,
) -> Result<(), EditError> {
    let page_number = reference_page(usage);
    let slot = reference_slot(usage);
    let container = reader.page(page_number)?;
    let original = reader.row_location(page_number, slot, 0, &container)?;
    if original.data.len() != QUALIFIED_ROW_BYTES || original.data[0] != INLINE_TAG {
        return Err(InspectError::Unsupported.into());
    }

    let mut members = allocation::pages(reader, usage)?;
    for &(prior_usage, prior_number) in &expected.reservations {
        reader.tick()?;
        if prior_usage == usage {
            push_member(&mut members, prior_number, reader)?;
        }
    }

    let mut minimum = number;
    let mut maximum = number;
    for &member in &members {
        reader.tick()?;
        minimum = minimum.min(member);
        maximum = maximum.max(member);
        if member < reader.header.page_count() {
            global.require_allocated(reader, member)?;
        }
    }

    reader.work(expected.rebased.len())?;
    let previous = expected.rebased.iter().position(|map| map.usage == usage);
    let base = match expected.policy {
        ReplayPolicy::Legacy => {
            if u64::from(maximum) - u64::from(minimum) < u64::from(INLINE_CAPACITY) {
                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
            }
            None
        }
        ReplayPolicy::CertifiedContinuation => {
            let current = if let Some(position) = previous {
                expected.rebased[position].base
            } else {
                let bytes: [u8; POINTER_BYTES] = original.data[1..PREFIX_BYTES]
                    .try_into()
                    .map_err(|_| EditError::Preservation)?;
                u32::from_le_bytes(bytes)
            };
            if number
                .checked_sub(current)
                .is_some_and(|relative| relative < INLINE_CAPACITY)
            {
                Some(current)
            } else {
                let aligned = minimum - minimum % u8::BITS;
                let span = u64::from(maximum) - u64::from(aligned) + 1;
                (span < u64::from(INLINE_CAPACITY)).then_some(aligned)
            }
        }
    };

    // The original row owns every before byte. A DATA-phase rewrite cannot be
    // retired or replaced by an index operation, including unchanged bytes.
    let end = original
        .offset
        .checked_add(original.data.len())
        .ok_or(EditError::Preservation)?;
    reader.work(expected.data_offsets.len() + expected.edits.len())?;
    if expected
        .data_offsets
        .iter()
        .any(|offset| original.offset <= *offset && *offset < end)
    {
        return Err(EditError::Preservation);
    }
    for edit in &expected.edits {
        if original.offset <= edit.offset
            && edit.offset < end
            && original.data[edit.offset - original.offset] != edit.before
        {
            return Err(EditError::Preservation);
        }
    }

    if let Some(base) = base {
        push_member(&mut members, number, reader)?;
        reader
            .budget
            .charge_allocation(QUALIFIED_ROW_BYTES as u64, 1)
            .map_err(InspectError::from)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(QUALIFIED_ROW_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        bytes.resize(QUALIFIED_ROW_BYTES, 0);
        bytes[1..PREFIX_BYTES].copy_from_slice(&base.to_le_bytes());
        for member in members {
            reader.tick()?;
            let relative = member.checked_sub(base).ok_or(EditError::Preservation)?;
            if relative >= INLINE_CAPACITY {
                return Err(EditError::Preservation);
            }
            let position = PREFIX_BYTES + relative as usize / u8::BITS as usize;
            let mask = 1 << (relative % u8::BITS);
            if bytes[position] & mask != 0 {
                return Err(EditError::Preservation);
            }
            bytes[position] |= mask;
        }

        reader
            .budget
            .charge_allocation(QUALIFIED_ROW_BYTES as u64, size_of::<Byte>() as u64)
            .map_err(InspectError::from)?;
        expected
            .edits
            .try_reserve(QUALIFIED_ROW_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        reader.work(expected.edits.len() + QUALIFIED_ROW_BYTES)?;
        expected
            .edits
            .retain(|edit| edit.offset < original.offset || edit.offset >= end);
        for (position, (&before, after)) in original.data.iter().zip(bytes).enumerate() {
            expected.edits.push(Byte {
                offset: original.offset + position,
                before,
                after,
            });
        }

        if let Some(position) = previous {
            expected.rebased[position].base = base;
        } else {
            reader
                .budget
                .charge_allocation(1, size_of::<Rebased>() as u64)
                .map_err(InspectError::from)?;
            expected
                .rebased
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            expected.rebased.push(Rebased {
                usage,
                base,
                row_offset: original.offset,
                original: original.data,
            });
        }

        return Ok(());
    }

    if maximum as usize >= PAGES_IN_BITMAP {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let metadata = number.checked_add(1).ok_or(EditError::Preservation)?;
    let cell = global.future_cell(reader, metadata)?;
    if cell.before & cell.mask == 0 {
        return Err(EditError::Preservation);
    }

    reader.work(expected.edits.len())?;
    expected
        .edits
        .retain(|edit| edit.offset < original.offset || edit.offset >= end);
    if let Some(position) = previous {
        reader.work(expected.rebased.len())?;
        expected.rebased.remove(position);
    }
    bit(&mut expected.edits, cell, false, reader)?;
    push_member(&mut members, number, reader)?;
    reader
        .budget
        .charge_allocation(1, size_of::<Promoted>() as u64)
        .map_err(InspectError::from)?;
    expected
        .promoted
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    expected.promoted.push(Promoted {
        usage,
        bitmap_page: metadata,
        row_offset: original.offset,
        original: original.data,
        members,
    });
    *next_eof = metadata;

    Ok(())
}

fn push_member(
    members: &mut Vec<u32>,
    number: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader
        .budget
        .charge_allocation(1, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    members
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    members.push(number);

    Ok(())
}
