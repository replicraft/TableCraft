//! Exact observed inline framing, separate from map-role rewrite authority.
use super::{INDIRECT_MAP, INLINE_BITMAP_OFFSET, INLINE_MAP, PAGE_POINTER_BYTES};
use crate::{
    InspectError, NativeReadSource,
    format::{
        bytes::u32_at,
        reader::Reader,
        row::{reference_page, reference_slot},
    },
};
use std::ops::Range;

const OBSERVED_BITMAP_BYTES: usize = 64;
const OBSERVED_RECORD_BYTES: usize = INLINE_BITMAP_OFFSET + OBSERVED_BITMAP_BYTES;
const MUTABLE_MAP_CONTAINER_OWNER: u32 = 0;

#[cfg(test)]
mod tests;

/// A before-image with the exact framing used by qualified map promotion.
/// Capturing it grants neither membership nor authority to rewrite its owner.
pub(crate) struct QualifiedInlineRecord {
    reference: u32,
    offset: usize,
    before: Vec<u8>,
}

impl QualifiedInlineRecord {
    pub(crate) const PAGE_CAPACITY: usize = OBSERVED_BITMAP_BYTES * u8::BITS as usize;

    pub(crate) fn capture<R: NativeReadSource>(
        reader: &mut Reader<'_, R>,
        reference: u32,
    ) -> Result<Self, InspectError> {
        let container = reference_page(reference);
        let slot = reference_slot(reference);
        let page = reader.page(container)?;
        let row = reader.row_location(container, slot, MUTABLE_MAP_CONTAINER_OWNER, &page)?;
        if row.data.len() != OBSERVED_RECORD_BYTES || row.data[0] != INLINE_MAP {
            return Err(InspectError::Unsupported);
        }

        Ok(Self {
            reference,
            offset: row.offset,
            before: row.data,
        })
    }

    pub(crate) fn reference(&self) -> u32 {
        self.reference
    }

    pub(crate) fn range(&self) -> Range<usize> {
        // The native row reader bounds this complete range in its input page.
        self.offset..self.offset + self.before.len()
    }

    pub(crate) fn before(&self) -> &[u8] {
        &self.before
    }

    pub(crate) fn start(&self) -> Result<u32, InspectError> {
        u32_at(&self.before, 1)
    }

    pub(crate) fn into_before(self) -> Vec<u8> {
        self.before
    }

    pub(crate) fn indirect<R: NativeReadSource>(
        &self,
        bitmap: u32,
        reader: &mut Reader<'_, R>,
    ) -> Result<Vec<u8>, InspectError> {
        let mut after = self.empty(reader)?;
        after[0] = INDIRECT_MAP;
        after[1..1 + PAGE_POINTER_BYTES].copy_from_slice(&bitmap.to_le_bytes());

        Ok(after)
    }

    /// Frame absolute members in a byte-aligned window without choosing policy.
    /// In particular, fitting this window does not authorize an owned/hint shift.
    pub(crate) fn inline<R: NativeReadSource>(
        &self,
        start: u32,
        members: &[u32],
        reader: &mut Reader<'_, R>,
    ) -> Result<Vec<u8>, InspectError> {
        reader.tick()?;
        if !start.is_multiple_of(u8::BITS) {
            return Err(InspectError::Unsupported);
        }
        start
            .checked_add(Self::PAGE_CAPACITY as u32 - 1)
            .ok_or(InspectError::Malformed)?;

        let mut after = self.empty(reader)?;
        after[0] = INLINE_MAP;
        after[1..1 + PAGE_POINTER_BYTES].copy_from_slice(&start.to_le_bytes());
        for &member in members {
            reader.tick()?;
            let relative = member
                .checked_sub(start)
                .filter(|relative| (*relative as usize) < Self::PAGE_CAPACITY)
                .ok_or(InspectError::Unsupported)?;
            let offset = INLINE_BITMAP_OFFSET + relative as usize / u8::BITS as usize;
            let mask = 1 << (relative % u8::BITS);
            if after[offset] & mask != 0 {
                return Err(InspectError::Malformed);
            }
            after[offset] |= mask;
        }

        Ok(after)
    }

    fn empty<R: NativeReadSource>(
        &self,
        reader: &mut Reader<'_, R>,
    ) -> Result<Vec<u8>, InspectError> {
        reader
            .budget
            .charge_allocation(self.before.len() as u64, 1)?;
        let mut after = Vec::new();
        after
            .try_reserve_exact(self.before.len())
            .map_err(|_| InspectError::Allocation)?;
        after.resize(self.before.len(), 0);
        Ok(after)
    }
}
