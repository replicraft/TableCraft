//! Per-operation EOF placement with separately owned bitmap metadata.
use super::{AddedPage, Plan, maps};
use crate::{
    EditError, IndexCheck, InspectError, NativeImage, NativeIndex, WriteBlocker,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        page::{PAGE_BYTES, USAGE_SIGNATURE},
        reader::Reader,
        row::{reference_page, reference_slot},
    },
    limits::MAX_IMAGE_BYTES,
    table::mutation::map_authority::{self, Owner},
};
use std::io::Cursor;

const INLINE_KIND: u8 = 0;
const INDIRECT_KIND: u8 = 1;
const POINTER_BYTES: usize = size_of::<u32>();
const INLINE_PREFIX_BYTES: usize = 1 + POINTER_BYTES;
const OBSERVED_INLINE_BITMAP_BYTES: usize = 64;
const OBSERVED_MAP_BYTES: usize = INLINE_PREFIX_BYTES + OBSERVED_INLINE_BITMAP_BYTES;
const BITMAP_PREFIX_BYTES: usize = USAGE_SIGNATURE.len();
const BITMAP_CAPACITY: usize = (PAGE_BYTES - BITMAP_PREFIX_BYTES) * u8::BITS as usize;

pub(in crate::table) struct Promotion {
    pub usage: u32,
    pub number: u32,
    pub offset: usize,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
    pub bitmap: Vec<u8>,
}

pub(super) fn reserve(
    image: &NativeImage,
    index: &NativeIndex,
    plan: &mut Plan,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<u32, EditError> {
    let number = reserve_inner(image, index, plan, reader)?;
    reader
        .budget
        .charge_allocation(1, size_of::<(u32, u32)>() as u64)
        .map_err(InspectError::from)?;
    plan.reservations
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    plan.reservations.push((index.usage_map(), number));
    Ok(number)
}

fn reserve_inner(
    image: &NativeImage,
    index: &NativeIndex,
    plan: &mut Plan,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<u32, EditError> {
    let number =
        u32::try_from(plan.length / PAGE_BYTES).map_err(|_| InspectError::StructureLimit)?;
    let next_length = advance(plan.length, 1)?;
    reader.work(plan.promotions.len())?;
    if let Some(promotion) = plan
        .promotions
        .iter_mut()
        .find(|p| p.usage == index.usage_map())
    {
        set_member(&mut promotion.bitmap, number, reader)?;
        global(image, number, &mut plan.map_edits, reader)?;
        plan.length = next_length;
        return Ok(number);
    }

    let owned = RepresentedMap::read(reader, index.usage_map(), MapRole::Owned)?;
    match owned.future_cell(reader, number) {
        Ok(_) => {
            maps::allocate_nonroot(
                image,
                index,
                number,
                &mut plan.map_edits,
                reader.budget,
                reader.cancelled,
            )?;
            plan.length = next_length;
            Ok(number)
        }
        Err(InspectError::Unsupported) if owned.kind() == INLINE_KIND => {
            promote(image, index, number, plan, reader)?;
            Ok(number)
        }
        Err(error) => Err(error.into()),
    }
}

fn promote(
    image: &NativeImage,
    index: &NativeIndex,
    number: u32,
    plan: &mut Plan,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let usage = index.usage_map();
    let container = reference_page(usage);
    let slot = reference_slot(usage);
    let page = reader.page(container)?;
    let row = reader.row_location(container, slot, 0, &page)?;
    if row.data.len() != OBSERVED_MAP_BYTES || row.data[0] != INLINE_KIND {
        return Err(InspectError::Unsupported.into());
    }
    let mut members = allocation::pages(reader, usage)?;
    for &(prior_usage, number) in &plan.reservations {
        reader.tick()?;
        if prior_usage == usage {
            reader
                .budget
                .charge_allocation(1, size_of::<u32>() as u64)
                .map_err(InspectError::from)?;
            members
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            members.push(number);
        }
    }
    reader.work(members.len().saturating_mul(2))?;
    let minimum = members
        .iter()
        .copied()
        .chain([number])
        .min()
        .ok_or(InspectError::Malformed)?;
    let maximum = members
        .iter()
        .copied()
        .chain([number])
        .max()
        .ok_or(InspectError::Malformed)?;
    let span = maximum
        .checked_sub(minimum)
        .and_then(|n| n.checked_add(1))
        .ok_or(InspectError::StructureLimit)?;
    let inline_capacity = (row.data.len() - INLINE_PREFIX_BYTES) * u8::BITS as usize;
    if span as usize <= inline_capacity || maximum as usize >= BITMAP_CAPACITY {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }

    let census = map_authority::capture_catalog(
        image,
        reader.budget,
        reader.cancelled,
        |_, _, _, _| Ok(()),
    )?;
    census.require_exclusive(
        &[
            Owner::GlobalFree,
            Owner::IndexOwned {
                definition: plan.owner,
                physical: index.data_number(),
            },
        ],
        reader.budget,
        reader.cancelled,
    )?;

    let metadata = number.checked_add(1).ok_or(InspectError::StructureLimit)?;
    let length = advance(plan.length, 2)?;
    let global_map = RepresentedMap::read(reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    global_map.require_allocated_storage(reader, &global_map)?;
    let owned = RepresentedMap::read(reader, usage, MapRole::Owned)?;
    owned.require_allocated_storage(reader, &global_map)?;
    for &member in &members {
        reader.tick()?;
        if member < image.catalog().header().page_count() {
            global_map.require_allocated(reader, member)?;
        }
    }
    reader
        .budget
        .charge_allocation(PAGE_BYTES as u64, 1)
        .map_err(InspectError::from)?;
    let mut bitmap = Vec::new();
    bitmap
        .try_reserve_exact(PAGE_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    bitmap.resize(PAGE_BYTES, 0);
    bitmap[..USAGE_SIGNATURE.len()].copy_from_slice(&USAGE_SIGNATURE);
    for member in members.into_iter().chain([number]) {
        set_member(&mut bitmap, member, reader)?;
    }
    reader
        .budget
        .charge_allocation(row.data.len() as u64, 1)
        .map_err(InspectError::from)?;
    let mut after = Vec::new();
    after
        .try_reserve_exact(row.data.len())
        .map_err(|_| InspectError::Allocation)?;
    after.resize(row.data.len(), 0);
    after[0] = INDIRECT_KIND;
    after[1..1 + POINTER_BYTES].copy_from_slice(&metadata.to_le_bytes());

    // Earlier children may have fit the old inline window. Their membership
    // now belongs to the complete promoted bitmap, never to the rewritten row.
    let record_end = row
        .offset
        .checked_add(row.data.len())
        .ok_or(InspectError::StructureLimit)?;
    reader.work(plan.map_edits.len())?;
    plan.map_edits
        .retain(|edit| edit.offset < row.offset || edit.offset >= record_end);

    global(image, number, &mut plan.map_edits, reader)?;
    global(image, metadata, &mut plan.map_edits, reader)?;
    reader
        .budget
        .charge_allocation(1, size_of::<Promotion>() as u64)
        .map_err(InspectError::from)?;
    plan.promotions
        .try_reserve(1)
        .map_err(|_| InspectError::Allocation)?;
    plan.promotions.push(Promotion {
        usage,
        number: metadata,
        offset: row.offset,
        before: row.data,
        after,
        bitmap,
    });
    plan.length = length;

    Ok(())
}

fn set_member(
    bitmap: &mut [u8],
    number: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.tick()?;
    if number as usize >= BITMAP_CAPACITY {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }

    let byte = BITMAP_PREFIX_BYTES + number as usize / u8::BITS as usize;
    let mask = 1 << (number % u8::BITS);
    if bitmap[byte] & mask != 0 {
        return Err(InspectError::Malformed.into());
    }
    bitmap[byte] |= mask;

    Ok(())
}

fn global(
    image: &NativeImage,
    number: u32,
    edits: &mut Vec<maps::Edit>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let mut source = image.reader();
    let mut original = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget: reader.budget,
        cancelled: reader.cancelled,
        reads: 0,
    };
    let global = RepresentedMap::read(&mut original, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    let cell = global.future_cell(&mut original, number)?;
    if cell.before & cell.mask == 0 {
        return Err(InspectError::Malformed.into());
    }
    maps::change(edits, cell, false, &mut original)
}

fn advance(length: usize, pages: usize) -> Result<usize, EditError> {
    length
        .checked_add(pages * PAGE_BYTES)
        .filter(|length| *length as u64 <= MAX_IMAGE_BYTES)
        .ok_or_else(|| InspectError::StructureLimit.into())
}

pub(super) fn seal(
    image: &NativeImage,
    indexes: &[IndexCheck],
    plan: &mut Plan,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    if !plan.promotions.is_empty() {
        let census = map_authority::capture_catalog(
            image,
            reader.budget,
            reader.cancelled,
            |_, _, _, _| Ok(()),
        )?;
        let mut owners = Vec::new();
        reader
            .budget
            .charge_allocation(
                (plan.reservations.len() + 1) as u64,
                size_of::<Owner>() as u64,
            )
            .map_err(InspectError::from)?;
        owners
            .try_reserve(plan.reservations.len() + 1)
            .map_err(|_| InspectError::Allocation)?;
        owners.push(Owner::GlobalFree);
        for &(usage, _) in &plan.reservations {
            reader.tick()?;
            reader.work(indexes.len() + owners.len())?;
            let index = indexes
                .iter()
                .find(|checked| checked.index().usage_map() == usage)
                .ok_or(EditError::Preservation)?;
            let owner = Owner::IndexOwned {
                definition: plan.owner,
                physical: index.index().data_number(),
            };
            if !owners.contains(&owner) {
                owners.push(owner);
            }
        }
        census.require_exclusive(&owners, reader.budget, reader.cancelled)?;
    }

    for promotion in &plan.promotions {
        reader.tick()?;
        reader
            .budget
            .charge_allocation(PAGE_BYTES as u64, 1)
            .map_err(InspectError::from)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(PAGE_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        bytes.extend_from_slice(&promotion.bitmap);
        reader
            .budget
            .charge_allocation(1, size_of::<AddedPage>() as u64)
            .map_err(InspectError::from)?;
        plan.added
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        plan.added.push(AddedPage {
            number: promotion.number,
            bytes,
        });
    }
    reader.work(plan.added.len().saturating_mul(plan.added.len()))?;
    plan.added.sort_unstable_by_key(|page| page.number);
    Ok(())
}
