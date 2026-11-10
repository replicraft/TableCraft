//! Sealed global-allocation changes for new physical pages, never page reclamation.
use super::Inventory;
use crate::{
    InspectError, NativeImage,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        bytes::{u16_at, u32_at},
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{Reader, row_count, row_range},
        row::{OFFSET_MASK, reference_page, reference_slot, slot_offset},
    },
    header::check,
    limits::MAX_READ_FILE_BYTES,
};
use std::{io::Cursor, ops::Range};
use tablecraft_core::limits::{LimitError, Resource, ResourceBudget};
use thiserror::Error;

const INLINE: u8 = 0;
const BITMAP_START: usize = 1 + size_of::<u32>();
const FREE_OFFSET: usize = DATA_SIGNATURE.len();

#[derive(Debug, Error)]
pub(crate) enum AllocationError {
    #[error(transparent)]
    Inspect(#[from] InspectError),
    #[error("native global allocation has no represented append capacity")]
    Capacity,
    #[error("native global allocation source is unsupported")]
    Source,
}

impl From<LimitError> for AllocationError {
    fn from(error: LimitError) -> Self {
        Self::Inspect(error.into())
    }
}

struct Change {
    bytes: Vec<u8>,
}

/// Exact allocator witness; its ranges never authorize another producer's bytes.
pub(crate) struct Allocation<'image> {
    image: &'image NativeImage,
    after_length: usize,
    ranges: Vec<Range<usize>>,
    changes: Vec<Change>,
}

impl<'image> Inventory<'image> {
    pub(crate) fn plan_allocation(
        &self,
        after_pages: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Allocation<'image>, AllocationError> {
        check(budget, cancelled)?;
        if after_pages < self.before_pages || after_pages == 0 {
            return Err(AllocationError::Source);
        }
        let after_length = u64::from(after_pages)
            .checked_mul(PAGE_BYTES as u64)
            .filter(|length| *length <= MAX_READ_FILE_BYTES)
            .and_then(|length| usize::try_from(length).ok())
            .ok_or(AllocationError::Capacity)?;
        let mut result = Allocation {
            image: self.image,
            after_length,
            ranges: Vec::new(),
            changes: Vec::new(),
        };
        let mut input = self.image.reader();
        let mut reader = Reader {
            source: &mut input,
            header: self.image.catalog().header(),
            budget,
            cancelled,
            reads: 0,
        };
        let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
        global.require_allocated_storage(&mut reader, &global)?;
        let host = reference_page(GLOBAL_FREE_REFERENCE);
        let slot = usize::from(reference_slot(GLOBAL_FREE_REFERENCE));
        let page = reader.page(host)?;
        let (word, start, end) = row_range(&page, slot)?;
        if word & !OFFSET_MASK != 0
            || page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || u32_at(&page, OWNER_OFFSET)? != host
        {
            return Err(AllocationError::Source);
        }
        let row = &page[start..end];
        let needs = if row.first() == Some(&INLINE) {
            let base = u32_at(row, 1)?;
            if base != 0 {
                return Err(AllocationError::Source);
            }
            BITMAP_START
                .checked_add((after_pages as usize).div_ceil(u8::BITS as usize))
                .ok_or(AllocationError::Capacity)?
        } else {
            0
        };
        if needs > row.len() {
            grow(
                &mut result,
                &page,
                Growth {
                    host,
                    slot,
                    start,
                    end,
                    needs,
                    before_pages: self.before_pages,
                    after_pages,
                },
                &mut reader,
            )?;
        } else {
            let mut previous = None;
            for page in self.before_pages..after_pages {
                reader.tick()?;
                let cell = global
                    .future_cell(&mut reader, page)
                    .map_err(|error| match error {
                        InspectError::Unsupported => AllocationError::Capacity,
                        error => AllocationError::Inspect(error),
                    })?;
                if cell.before & cell.mask == 0 {
                    continue;
                }
                let position = match previous {
                    Some((offset, position)) if offset == cell.offset => position,
                    _ => {
                        let position = result.changes.len();
                        result.push(cell.offset..cell.offset + 1, &[cell.before], &mut reader)?;
                        previous = Some((cell.offset, position));
                        position
                    }
                };
                result.changes[position].bytes[0] &= !cell.mask;
            }
        }
        Ok(result)
    }
}

struct Growth {
    host: u32,
    slot: usize,
    start: usize,
    end: usize,
    needs: usize,
    before_pages: u32,
    after_pages: u32,
}

fn grow(
    result: &mut Allocation<'_>,
    page: &[u8; PAGE_BYTES],
    growth: Growth,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), AllocationError> {
    let Growth {
        host,
        slot,
        start,
        end,
        needs,
        before_pages,
        after_pages,
    } = growth;
    // The global packed reference remains unchanged. Only a sole direct row can
    // grow into this free gap without moving an unrelated metadata record.
    let count = row_count(page)?;
    if page[8..12] != [0; 4] {
        return Err(AllocationError::Source);
    }
    if slot != 0 || count != 1 || end != PAGE_BYTES {
        return Err(AllocationError::Capacity);
    }
    let directory = slot_offset(count);
    let free = start
        .checked_sub(directory)
        .ok_or(AllocationError::Source)?;
    if usize::from(u16_at(page, FREE_OFFSET)?) != free {
        return Err(AllocationError::Source);
    }
    let new_start = end
        .checked_sub(needs)
        .filter(|value| *value >= directory)
        .ok_or(AllocationError::Capacity)?;
    reader.budget.charge_allocation(needs as u64, 1)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(needs)
        .map_err(|_| InspectError::Allocation)?;
    bytes.extend_from_slice(&page[start..end]);
    bytes.resize(needs, 0);
    for number in before_pages..after_pages {
        reader.tick()?;
        let byte = BITMAP_START + number as usize / u8::BITS as usize;
        bytes[byte] &= !(1 << (number % u8::BITS));
    }
    let base = host as usize * PAGE_BYTES;
    result.push(
        base + FREE_OFFSET..base + FREE_OFFSET + size_of::<u16>(),
        &((new_start - directory) as u16).to_le_bytes(),
        reader,
    )?;
    result.push(
        base + slot_offset(slot)..base + slot_offset(slot) + size_of::<u16>(),
        &(new_start as u16).to_le_bytes(),
        reader,
    )?;
    result.push(base + new_start..base + end, &bytes, reader)?;
    Ok(())
}

impl Allocation<'_> {
    fn push(
        &mut self,
        range: Range<usize>,
        bytes: &[u8],
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), AllocationError> {
        reader.tick()?;
        reader.work(self.ranges.len())?;
        if range.len() != bytes.len()
            || range.end > self.image.as_bytes().len()
            || self
                .ranges
                .iter()
                .any(|old| old.start < range.end && range.start < old.end)
        {
            return Err(AllocationError::Source);
        }
        reader
            .budget
            .charge_allocation(1, (size_of::<Range<usize>>() + size_of::<Change>()) as u64)?;
        reader.budget.charge_allocation(bytes.len() as u64, 1)?;
        self.ranges
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        self.changes
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(bytes.len())
            .map_err(|_| InspectError::Allocation)?;
        owned.extend_from_slice(bytes);
        self.ranges.push(range);
        self.changes.push(Change { bytes: owned });
        Ok(())
    }

    pub(crate) fn allowed_before_ranges(&self) -> &[Range<usize>] {
        &self.ranges
    }

    pub(crate) fn apply(
        &self,
        candidate: &mut [u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), AllocationError> {
        if candidate.len() != self.after_length {
            return Err(AllocationError::Source);
        }
        // Validate all source witnesses before changing even the first byte.
        for range in &self.ranges {
            check(budget, cancelled)?;
            budget.charge(Resource::Steps, range.len() as u64)?;
            if candidate.get(range.start..range.end)
                != self.image.as_bytes().get(range.start..range.end)
            {
                return Err(AllocationError::Source);
            }
        }
        for (range, change) in self.ranges.iter().zip(&self.changes) {
            check(budget, cancelled)?;
            budget.charge(Resource::Steps, range.len() as u64)?;
            candidate[range.start..range.end].copy_from_slice(&change.bytes);
        }
        Ok(())
    }

    pub(crate) fn verify_after(
        &self,
        candidate: &[u8],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<(), AllocationError> {
        if candidate.len() != self.after_length {
            return Err(AllocationError::Source);
        }
        for (range, change) in self.ranges.iter().zip(&self.changes) {
            check(budget, cancelled)?;
            budget.charge(Resource::Steps, range.len() as u64)?;
            if candidate.get(range.start..range.end) != Some(change.bytes.as_slice()) {
                return Err(AllocationError::Source);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
