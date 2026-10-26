//! Random membership over fully validated usage metadata, retaining one bitmap.
use super::{Kind, PageCursor, read_bitmap, slot_base};
use crate::{
    InspectError, NativeReadSource,
    format::{
        allocation::{
            INDIRECT_BITMAP_OFFSET, INLINE_BITMAP_OFFSET, PAGE_POINTER_BYTES, PAGES_PER_BITMAP,
            walk_bitmap,
        },
        bitmap,
        bytes::u32_at,
        page::PAGE_BYTES,
        reader::Reader,
        row::MASK_BITS,
    },
};

// The one-element fallible allocation keeps the bitmap off the indexed reader's
// stack. Its size is prepaid; no per-address collection is constructed.
struct State {
    map: Vec<u8>,
    kind: Kind,
    page: [u8; PAGE_BYTES],
    base: Option<u32>,
}

pub(crate) struct PageMembership {
    state: Vec<State>,
}

impl PageMembership {
    pub(crate) fn new<R: NativeReadSource>(
        reader: &mut Reader<'_, R>,
        reference: u32,
    ) -> Result<Self, InspectError> {
        let cursor = PageCursor::new(reader, reference)?;
        reader.tick()?;
        reader
            .budget
            .charge_allocation(1, size_of::<State>() as u64)?;
        let mut state = Vec::new();
        state
            .try_reserve_exact(1)
            .map_err(|_| InspectError::Allocation)?;
        state.push(State {
            map: cursor.map,
            kind: cursor.kind,
            page: cursor.page,
            base: cursor.base,
        });
        Ok(Self { state })
    }

    pub(crate) fn contains<R: NativeReadSource>(
        &mut self,
        reader: &mut Reader<'_, R>,
        number: u32,
    ) -> Result<bool, InspectError> {
        reader.tick()?;
        if number >= reader.header.page_count() {
            return Err(InspectError::Malformed);
        }
        let state = self.state.first_mut().ok_or(InspectError::Malformed)?;
        let (bytes, relative) = match state.kind {
            Kind::Inline => {
                let base = state.base.ok_or(InspectError::Malformed)?;
                let Some(relative) = number.checked_sub(base) else {
                    return Ok(false);
                };
                (&state.map[INLINE_BITMAP_OFFSET..], relative as usize)
            }
            Kind::Indirect => {
                let slot = number as usize / PAGES_PER_BITMAP;
                let slots = (state.map.len() - 1) / PAGE_POINTER_BYTES;
                if slot >= slots {
                    return Ok(false);
                }
                let pointer = u32_at(&state.map, 1 + slot * PAGE_POINTER_BYTES)?;
                if pointer == 0 {
                    return Ok(false);
                }
                let base = slot_base(slot)?;
                if state.base != Some(base) {
                    let page = read_bitmap(reader, pointer)?;
                    let mut last = None;
                    walk_bitmap(
                        reader,
                        &page[INDIRECT_BITMAP_OFFSET..],
                        base,
                        &mut last,
                        |_, _| Ok(()),
                    )?;
                    reader.tick()?;
                    state.page = page;
                    state.base = Some(base);
                }
                (
                    &state.page[INDIRECT_BITMAP_OFFSET..],
                    number as usize % PAGES_PER_BITMAP,
                )
            }
        };
        let byte = relative / MASK_BITS;
        let within = relative % MASK_BITS;
        reader.work(1)?;
        Ok(bytes.get(byte).is_some_and(|byte| {
            bitmap::set_bits(std::slice::from_ref(byte)).any(|bit| bit == within)
        }))
    }
}

#[cfg(test)]
mod tests;
