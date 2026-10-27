//! Independently replay EOF purposes, role bytes and bitmap promotion.
use super::super::Plan;
use crate::{
    EditError, IndexCheck, InspectError, NativeImage,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapCell, MapRole, RepresentedMap},
        page::{PAGE_BYTES, USAGE_SIGNATURE},
        reader::Reader,
    },
    table::mutation::{
        CertifiedData,
        map_authority::{self, Owner, RecordIdentity},
    },
};
use std::io::Cursor;

mod inline;

const INLINE_TAG: u8 = 0;
const INDIRECT_TAG: u8 = 1;
const POINTER_BYTES: usize = size_of::<u32>();
const PREFIX_BYTES: usize = 1 + POINTER_BYTES;
const QUALIFIED_INLINE_BITMAP_BYTES: usize = 64;
const QUALIFIED_ROW_BYTES: usize = PREFIX_BYTES + QUALIFIED_INLINE_BITMAP_BYTES;
const USAGE_PREFIX_BYTES: usize = 4;
const PAGES_IN_BITMAP: usize = (PAGE_BYTES - USAGE_PREFIX_BYTES) * u8::BITS as usize;

struct Byte {
    offset: usize,
    before: u8,
    after: u8,
}

struct Promoted {
    usage: u32,
    bitmap_page: u32,
    row_offset: usize,
    original: Vec<u8>,
    members: Vec<u32>,
}

#[derive(Default)]
enum ReplayPolicy {
    #[default]
    Legacy,
    CertifiedContinuation,
}

struct Rebased {
    usage: u32,
    base: u32,
    row_offset: usize,
    original: Vec<u8>,
}

#[derive(Default)]
pub(super) struct Expected {
    edits: Vec<Byte>,
    data_offsets: Vec<usize>,
    promoted: Vec<Promoted>,
    reservations: Vec<(u32, u32)>,
    rebased: Vec<Rebased>,
    policy: ReplayPolicy,
}

pub(super) fn seed(
    image: &NativeImage,
    data: &CertifiedData<'_>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Expected, EditError> {
    data.require_source(image)?;
    let mut expected = Expected {
        policy: ReplayPolicy::CertifiedContinuation,
        ..Expected::default()
    };
    for (offset, before, after) in data.edits() {
        reader.tick()?;
        reader.work(expected.edits.len())?;
        if image.as_bytes().get(offset) != Some(&before)
            || expected.edits.iter().any(|prior| prior.offset == offset)
        {
            return Err(EditError::Preservation);
        }

        reader
            .budget
            .charge_allocation(1, (size_of::<Byte>() + size_of::<usize>()) as u64)
            .map_err(InspectError::from)?;
        expected
            .edits
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        expected
            .data_offsets
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        expected.edits.push(Byte {
            offset,
            before,
            after,
        });
        expected.data_offsets.push(offset);
    }

    // DATA membership must never participate in late IndexOwned promotion.
    // Only independent index reserve() calls populate reservations below.
    Ok(expected)
}

fn bit(
    expected: &mut Vec<Byte>,
    cell: MapCell,
    set: bool,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.work(expected.len())?;
    if let Some(prior) = expected
        .iter_mut()
        .find(|prior| prior.offset == cell.offset)
    {
        if prior.before != cell.before {
            return Err(EditError::Preservation);
        }
        prior.after = if set {
            prior.after | cell.mask
        } else {
            prior.after & !cell.mask
        };
    } else {
        reader
            .budget
            .charge_allocation(1, size_of::<Byte>() as u64)
            .map_err(InspectError::from)?;
        expected
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        expected.push(Byte {
            offset: cell.offset,
            before: cell.before,
            after: if set {
                cell.before | cell.mask
            } else {
                cell.before & !cell.mask
            },
        });
    }
    Ok(())
}

pub(super) fn reserve(
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    usage: u32,
    next_eof: &mut u32,
    expected: &mut Expected,
) -> Result<u32, EditError> {
    let number = *next_eof;
    let global = RepresentedMap::read(reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    global.require_allocated_storage(reader, &global)?;
    let owned = RepresentedMap::read(reader, usage, MapRole::Owned)?;
    owned.require_allocated_storage(reader, &global)?;
    reader.work(expected.promoted.len())?;
    if let Some(map) = expected.promoted.iter_mut().find(|map| map.usage == usage) {
        if number as usize >= PAGES_IN_BITMAP || map.members.contains(&number) {
            return Err(EditError::Preservation);
        }
        reader.work(map.members.len())?;
        reader
            .budget
            .charge_allocation(1, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        map.members
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        map.members.push(number);
    } else {
        reader.work(expected.rebased.len())?;
        let rewritten = expected.rebased.iter().any(|map| map.usage == usage);
        if rewritten {
            inline::reserve(reader, usage, number, next_eof, expected, &global)?;
        } else {
            match owned.future_cell(reader, number) {
                Ok(cell) => {
                    if cell.before & cell.mask != 0 {
                        return Err(EditError::Preservation);
                    }
                    bit(&mut expected.edits, cell, true, reader)?;
                }
                Err(InspectError::Unsupported) if owned.kind() == INLINE_TAG => {
                    inline::reserve(reader, usage, number, next_eof, expected, &global)?;
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
    let free = global.future_cell(reader, number)?;
    if free.before & free.mask == 0 {
        return Err(EditError::Preservation);
    }
    bit(&mut expected.edits, free, false, reader)?;
    reader
        .budget
        .charge_allocation(1, size_of::<(u32, u32)>() as u64)
        .map_err(InspectError::from)?;
    expected
        .reservations
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    expected.reservations.push((usage, number));
    *next_eof = next_eof.checked_add(1).ok_or(EditError::Preservation)?;
    Ok(number)
}

pub(super) fn finish(
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    expected: &Expected,
    plan: &Plan,
    after: &[u8],
) -> Result<(), EditError> {
    if expected.edits.len() != plan.append.map_edits.len()
        || expected.promoted.len() != plan.promotions.len()
        || expected.rebased.len() != plan.rebases.len()
    {
        return Err(EditError::Preservation);
    }
    for byte in &expected.edits {
        reader.tick()?;
        reader.work(plan.append.map_edits.len())?;
        let matching = plan
            .append
            .map_edits
            .iter()
            .find(|edit| edit.offset == byte.offset)
            .ok_or(EditError::Preservation)?;
        if matching.before != byte.before
            || matching.after != byte.after
            || after.get(byte.offset) != Some(&byte.after)
        {
            return Err(EditError::Preservation);
        }
    }
    for map in &expected.rebased {
        reader.tick()?;
        reader.work(plan.rebases.len())?;
        let actual = plan
            .rebases
            .iter()
            .find(|actual| actual.usage == map.usage)
            .ok_or(EditError::Preservation)?;
        if actual.base != map.base {
            return Err(EditError::Preservation);
        }
    }

    for map in &expected.promoted {
        reader.tick()?;
        reader.work(plan.promotions.len())?;
        let actual = plan
            .promotions
            .iter()
            .find(|actual| actual.usage == map.usage)
            .ok_or(EditError::Preservation)?;
        if actual.number != map.bitmap_page
            || actual.offset != map.row_offset
            || actual.before != map.original
            || actual.after.len() != map.original.len()
        {
            return Err(EditError::Preservation);
        }
        let end = map
            .row_offset
            .checked_add(map.original.len())
            .ok_or(EditError::Preservation)?;
        let record = after
            .get(map.row_offset..end)
            .ok_or(EditError::Preservation)?;
        for (position, &byte) in record.iter().enumerate() {
            reader.tick()?;
            let expected = match position {
                0 => INDIRECT_TAG,
                1..=POINTER_BYTES => map.bitmap_page.to_le_bytes()[position - 1],
                _ => 0,
            };
            if byte != expected || actual.after[position] != expected {
                return Err(EditError::Preservation);
            }
        }
        let start = map.bitmap_page as usize * PAGE_BYTES;
        let bitmap = after
            .get(start..start + PAGE_BYTES)
            .ok_or(EditError::Preservation)?;
        // Independently derive every bitmap byte, including all unused bytes.
        // Its usage tag cannot be an ordinary DATA or LVAL target.
        for (position, &byte) in bitmap.iter().enumerate() {
            reader.tick()?;
            let mut expected = USAGE_SIGNATURE.get(position).copied().unwrap_or(0);
            if position >= USAGE_PREFIX_BYTES {
                reader.work(map.members.len())?;
                expected = 0;
                for &member in &map.members {
                    if member as usize / u8::BITS as usize == position - USAGE_PREFIX_BYTES {
                        expected |= 1 << (member % u8::BITS);
                    }
                }
            }
            if byte != expected || actual.bitmap.get(position) != Some(&expected) {
                return Err(EditError::Preservation);
            }
        }
    }
    Ok(())
}

pub(super) fn cover_suffix(
    reader: &mut Reader<'_, Cursor<&[u8]>>,
    expected: &Expected,
    data: &CertifiedData<'_>,
    end: u32,
) -> Result<(), EditError> {
    let count = data
        .pages()
        .len()
        .checked_add(expected.reservations.len())
        .and_then(|count| count.checked_add(expected.promoted.len()))
        .ok_or(EditError::Preservation)?;
    let original = reader.header.page_count();
    if usize::try_from(end.checked_sub(original).ok_or(EditError::Preservation)?)
        .map_err(|_| EditError::Preservation)?
        != count
    {
        return Err(EditError::Preservation);
    }

    reader
        .budget
        .charge_allocation(count as u64, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    let mut purposes = Vec::new();
    purposes
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    for &number in data.pages() {
        reader.tick()?;
        purposes.push(number);
    }
    for &(_, number) in &expected.reservations {
        reader.tick()?;
        purposes.push(number);
    }
    for promotion in &expected.promoted {
        reader.tick()?;
        purposes.push(promotion.bitmap_page);
    }

    let levels = count.checked_ilog2().unwrap_or(0) as usize + 1;
    reader.work(count.saturating_mul(levels))?;
    purposes.sort_unstable();
    for (position, number) in purposes.into_iter().enumerate() {
        reader.tick()?;
        let position = u32::try_from(position).map_err(|_| EditError::Preservation)?;
        if original.checked_add(position) != Some(number) {
            return Err(EditError::Preservation);
        }
    }

    Ok(())
}

pub(super) fn authority(
    image: &NativeImage,
    definition: u32,
    indexes: &[IndexCheck],
    plan: &Plan,
    expected: &Expected,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    if expected.promoted.is_empty() && expected.rebased.is_empty() {
        return Ok(());
    }
    let census = map_authority::capture_catalog(
        image,
        reader.budget,
        reader.cancelled,
        |_, _, _, _| Ok(()),
    )?;
    let mut selected = Vec::new();
    reader
        .budget
        .charge_allocation(
            (expected.reservations.len() + 1) as u64,
            size_of::<Owner>() as u64,
        )
        .map_err(InspectError::from)?;
    selected
        .try_reserve(expected.reservations.len() + 1)
        .map_err(|_| InspectError::Allocation)?;
    selected.push(Owner::GlobalFree);
    for &(usage, _) in &expected.reservations {
        reader.tick()?;
        reader.work(indexes.len() + selected.len())?;
        let index = indexes
            .iter()
            .find(|checked| checked.index().usage_map() == usage)
            .ok_or(EditError::Preservation)?;
        let owner = Owner::IndexOwned {
            definition,
            physical: index.index().data_number(),
        };
        if !selected.contains(&owner) {
            selected.push(owner);
        }
    }
    census.require_exclusive(&selected, reader.budget, reader.cancelled)?;

    for (usage, row_offset, original) in expected
        .promoted
        .iter()
        .map(|map| (map.usage, map.row_offset, map.original.as_slice()))
        .chain(
            expected
                .rebased
                .iter()
                .map(|map| (map.usage, map.row_offset, map.original.as_slice())),
        )
    {
        reader.tick()?;
        reader.work(indexes.len() + plan.rebases.len())?;
        let index = indexes
            .iter()
            .find(|checked| checked.index().usage_map() == usage)
            .ok_or(EditError::Preservation)?;
        let physical = index.index().data_number();
        if plan
            .rebases
            .iter()
            .any(|map| map.usage == usage && map.physical != physical)
        {
            return Err(EditError::Preservation);
        }

        let owner = Owner::IndexOwned {
            definition,
            physical,
        };
        let end = row_offset
            .checked_add(original.len())
            .ok_or(EditError::Preservation)?;
        census.require_record_rewrite(
            &selected,
            RecordIdentity {
                owner,
                reference: usage,
                range: row_offset..end,
            },
            reader.budget,
            reader.cancelled,
        )?;
    }

    Ok(())
}
