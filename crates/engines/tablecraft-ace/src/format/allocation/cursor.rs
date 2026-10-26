//! Validate complete usage metadata, then enumerate page addresses without a list.
mod membership;
use super::{
    INDIRECT_BITMAP_OFFSET, INDIRECT_MAP, INLINE_BITMAP_OFFSET, INLINE_MAP, PAGE_POINTER_BYTES,
    PAGES_PER_BITMAP, checked_page, walk_bitmap,
};
use crate::{
    InspectError,
    format::{
        bitmap,
        bytes::u32_at,
        page::{PAGE_BYTES, USAGE_SIGNATURE},
        reader::Reader,
        row::{reference_page, reference_slot},
    },
};
pub(crate) use membership::PageMembership;

#[derive(Clone, Copy)]
enum Kind {
    Inline,
    Indirect,
}

// Retains the bounded usage row and at most one bitmap page, not one u32 per
// allocated page. All metadata is validated before the caller can read records.
pub(crate) struct PageCursor {
    map: Vec<u8>,
    kind: Kind,
    page: [u8; PAGE_BYTES],
    base: Option<u32>,
    next_slot: usize,
    next_block: usize,
    pending: bitmap::SetBits,
    pending_offset: usize,
    last: Option<u32>,
}
impl PageCursor {
    pub(crate) fn new<R: crate::NativeReadSource>(
        reader: &mut Reader<'_, R>,
        reference: u32,
    ) -> Result<Self, InspectError> {
        let map = reader.row(reference_page(reference), reference_slot(reference), None)?;
        let mut cursor = Self {
            map,
            kind: Kind::Inline,
            page: [0; PAGE_BYTES],
            base: None,
            next_slot: 0,
            next_block: 0,
            pending: bitmap::set_bits(&[]),
            pending_offset: 0,
            last: None,
        };
        let mut last = None;
        match cursor.map.first().copied() {
            Some(INLINE_MAP) => {
                let base = u32_at(&cursor.map, 1)?;
                walk_bitmap(
                    reader,
                    cursor
                        .map
                        .get(INLINE_BITMAP_OFFSET..)
                        .ok_or(InspectError::Truncated)?,
                    base,
                    &mut last,
                    |_, _| Ok(()),
                )?;
                cursor.base = Some(base);
            }
            Some(INDIRECT_MAP) => {
                cursor.kind = Kind::Indirect;
                if !(cursor.map.len() - 1).is_multiple_of(PAGE_POINTER_BYTES) {
                    return Err(InspectError::Malformed);
                }
                let slots = (cursor.map.len() - 1) / PAGE_POINTER_BYTES;
                cursor.next_slot = slots;
                for slot in 0..slots {
                    reader.tick()?;
                    let pointer = u32_at(&cursor.map, 1 + slot * PAGE_POINTER_BYTES)?;
                    if pointer == 0 {
                        continue;
                    }
                    let page = read_bitmap(reader, pointer)?;
                    let base = slot_base(slot)?;
                    let previous = last;
                    walk_bitmap(
                        reader,
                        &page[INDIRECT_BITMAP_OFFSET..],
                        base,
                        &mut last,
                        |_, _| Ok(()),
                    )?;
                    if cursor.base.is_none() && previous != last {
                        cursor.page = page;
                        cursor.base = Some(base);
                        cursor.next_slot = slot + 1;
                    }
                }
            }
            _ => return Err(InspectError::Unsupported),
        }
        Ok(cursor)
    }
    pub(crate) fn next<R: crate::NativeReadSource>(
        &mut self,
        reader: &mut Reader<'_, R>,
    ) -> Result<Option<u32>, InspectError> {
        reader.tick()?;
        loop {
            if let Some(bit) = self.pending.next() {
                let base = self.base.ok_or(InspectError::Malformed)?;
                return checked_page(
                    base,
                    self.pending_offset,
                    bit,
                    reader.header.page_count(),
                    &mut self.last,
                )
                .map(Some);
            }
            if self.base.is_some() {
                let bytes = match self.kind {
                    Kind::Inline => &self.map[INLINE_BITMAP_OFFSET..],
                    Kind::Indirect => &self.page[INDIRECT_BITMAP_OFFSET..],
                };
                if self.next_block < bytes.len() {
                    let end = (self.next_block + bitmap::BLOCK_BYTES).min(bytes.len());
                    reader.work(end - self.next_block)?;
                    self.pending = bitmap::set_bits(&bytes[self.next_block..end]);
                    self.pending_offset = self.next_block;
                    self.next_block = end;
                    continue;
                }
            }
            self.base = None;
            match self.kind {
                Kind::Inline => return Ok(None),
                Kind::Indirect => {
                    let slots = (self.map.len() - 1) / PAGE_POINTER_BYTES;
                    while self.next_slot < slots {
                        reader.tick()?;
                        let slot = self.next_slot;
                        self.next_slot += 1;
                        let pointer = u32_at(&self.map, 1 + slot * PAGE_POINTER_BYTES)?;
                        if pointer == 0 {
                            continue;
                        }
                        let page = read_bitmap(reader, pointer)?;
                        let base = slot_base(slot)?;
                        // Source ownership must stay stable/offline. Revalidate a
                        // reread bitmap before yielding from this buffer as well.
                        let mut last = self.last;
                        walk_bitmap(
                            reader,
                            &page[INDIRECT_BITMAP_OFFSET..],
                            base,
                            &mut last,
                            |_, _| Ok(()),
                        )?;
                        self.page = page;
                        self.base = Some(base);
                        self.next_block = 0;
                        break;
                    }
                    if self.base.is_none() {
                        return Ok(None);
                    }
                }
            }
        }
    }
}
fn slot_base(slot: usize) -> Result<u32, InspectError> {
    slot.checked_mul(PAGES_PER_BITMAP)
        .and_then(|base| base.try_into().ok())
        .ok_or(InspectError::StructureLimit)
}
fn read_bitmap<R: crate::NativeReadSource>(
    reader: &mut Reader<'_, R>,
    pointer: u32,
) -> Result<[u8; PAGE_BYTES], InspectError> {
    let page = reader.page(pointer)?;
    if page[..USAGE_SIGNATURE.len()] != USAGE_SIGNATURE {
        return Err(InspectError::Malformed);
    }
    Ok(page)
}

#[cfg(test)]
mod tests;
