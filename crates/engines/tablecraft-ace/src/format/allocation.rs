use super::{bitmap, bytes::u32_at, reader::Reader};
use super::{
    page::{PAGE_BYTES, USAGE_SIGNATURE},
    row::{MASK_BITS, reference_page, reference_slot},
};
use crate::InspectError;
use crate::profile::MAX_MAPPED_PAGES;
use std::io::{Read, Seek};

const INLINE_MAP: u8 = 0;
const INDIRECT_MAP: u8 = 1;
const PAGE_POINTER_BYTES: usize = size_of::<u32>();
const INLINE_BITMAP_OFFSET: usize = 1 + PAGE_POINTER_BYTES;
const INDIRECT_BITMAP_OFFSET: usize = 4;
const PAGES_PER_BITMAP: usize = (PAGE_BYTES - INDIRECT_BITMAP_OFFSET) * MASK_BITS;

pub(crate) fn pages<R: Read + Seek>(
    reader: &mut Reader<'_, R>,
    reference: u32,
) -> Result<Vec<u32>, InspectError> {
    let map = reader.row(reference_page(reference), reference_slot(reference), None)?;
    let mut pages = Vec::new();
    match map.first().copied() {
        Some(INLINE_MAP) => bitmap(
            reader,
            map.get(INLINE_BITMAP_OFFSET..)
                .ok_or(InspectError::Truncated)?,
            u32_at(&map, 1)?,
            &mut pages,
        )?,
        Some(INDIRECT_MAP) => {
            if !(map.len() - 1).is_multiple_of(PAGE_POINTER_BYTES) {
                return Err(InspectError::Malformed);
            }
            for (index, bytes) in map[1..]
                .as_chunks::<PAGE_POINTER_BYTES>()
                .0
                .iter()
                .enumerate()
            {
                reader.tick()?;
                let pointer = u32_at(bytes, 0)?;
                if pointer == 0 {
                    continue;
                }
                let page = reader.page(pointer)?;
                if page[..USAGE_SIGNATURE.len()] != USAGE_SIGNATURE {
                    return Err(InspectError::Malformed);
                }
                bitmap(
                    reader,
                    &page[INDIRECT_BITMAP_OFFSET..],
                    (index * PAGES_PER_BITMAP)
                        .try_into()
                        .map_err(|_| InspectError::StructureLimit)?,
                    &mut pages,
                )?;
            }
        }
        _ => return Err(InspectError::Unsupported),
    }
    Ok(pages)
}
fn bitmap<R: Read + Seek>(
    reader: &mut Reader<'_, R>,
    bytes: &[u8],
    base: u32,
    pages: &mut Vec<u32>,
) -> Result<(), InspectError> {
    for (block, bytes) in bytes.chunks(bitmap::BLOCK_BYTES).enumerate() {
        reader.work(bytes.len())?;
        for bit in bitmap::set_bits(bytes) {
            let page = base
                .checked_add(
                    (block * bitmap::BLOCK_BYTES * MASK_BITS + bit)
                        .try_into()
                        .map_err(|_| InspectError::StructureLimit)?,
                )
                .ok_or(InspectError::Malformed)?;
            if page >= reader.header.page_count() {
                return Err(InspectError::Malformed);
            }
            // Bits are emitted in ascending order within each block. Indirect
            // slots cover disjoint ascending PAGES_PER_BITMAP intervals, so the
            // last entry establishes uniqueness/order without a quadratic scan.
            if pages.last().is_some_and(|last| *last >= page) {
                return Err(InspectError::Malformed);
            }
            if pages.len() >= MAX_MAPPED_PAGES {
                return Err(InspectError::StructureLimit);
            }
            reader.budget.charge(
                tablecraft_core::limits::Resource::Bytes,
                PAGE_POINTER_BYTES as u64,
            )?;
            pages.try_reserve(1).map_err(|_| InspectError::Allocation)?;
            pages.push(page);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
