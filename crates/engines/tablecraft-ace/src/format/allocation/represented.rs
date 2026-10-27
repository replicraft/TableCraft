//! Existing represented cells only; free-map polarity is never owned-map scope.
use super::{
    INDIRECT_BITMAP_OFFSET, INDIRECT_MAP, INLINE_BITMAP_OFFSET, INLINE_MAP, PAGE_POINTER_BYTES,
    PAGES_PER_BITMAP, PageCursor,
};
use crate::{
    InspectError, NativeReadSource,
    format::{
        bytes::u32_at,
        page::{OWNER_OFFSET, PAGE_BYTES, USAGE_SIGNATURE},
        reader::{Reader, row_range},
        row::{OFFSET_MASK, reference, reference_page, reference_slot},
    },
};

const GLOBAL_MAP_PAGE: u32 = 1;
const GLOBAL_MAP_SLOT: u8 = 0;
const OWNED_CONTAINER_OWNER: u32 = 0;
pub(crate) const GLOBAL_FREE_REFERENCE: u32 = reference(GLOBAL_MAP_PAGE, GLOBAL_MAP_SLOT);

#[derive(Clone, Copy)]
pub(crate) enum MapRole {
    Owned,
    GlobalFree,
    // Candidate data pages, not a complete free-byte or ownership inventory.
    #[cfg(feature = "ace16-write-preview")]
    TableFreeSpace,
    // Column-specific candidate pages, selected through qualified LVAL metadata.
    LongValueCandidate,
}

pub(crate) struct MapCell {
    pub offset: usize,
    pub before: u8,
    pub mask: u8,
}

pub(crate) struct RepresentedMap {
    payload: Vec<u8>,
    offset: usize,
    container: u32,
    role: MapRole,
}

impl RepresentedMap {
    pub(crate) fn kind(&self) -> u8 {
        self.payload[0]
    }

    /// Retain physical metadata identities for an independent unchanged-page audit.
    #[cfg(feature = "ace16-write-preview")]
    pub(crate) fn protect_storage<R: NativeReadSource>(
        &self,
        reader: &mut Reader<'_, R>,
        protected: &mut Vec<u32>,
    ) -> Result<(), InspectError> {
        let mut append = |number: u32, reader: &mut Reader<'_, R>| {
            reader.work(protected.len())?;
            if !protected.contains(&number) {
                reader
                    .budget
                    .charge_allocation(1, size_of::<u32>() as u64)?;
                protected
                    .try_reserve(1)
                    .map_err(|_| InspectError::Allocation)?;
                protected.push(number);
            }
            Ok::<(), InspectError>(())
        };
        append(self.container, reader)?;
        if self.payload[0] == INDIRECT_MAP {
            for bytes in self.payload[1..].as_chunks::<PAGE_POINTER_BYTES>().0 {
                reader.tick()?;
                let number = u32_at(bytes, 0)?;
                if number != 0 {
                    append(number, reader)?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn read<R: NativeReadSource>(
        reader: &mut Reader<'_, R>,
        reference: u32,
        role: MapRole,
    ) -> Result<Self, InspectError> {
        let number = reference_page(reference);
        let slot = reference_slot(reference);
        let page = reader.page(number)?;
        let (flags, _, _) = row_range(&page, usize::from(slot))?;
        if flags & !OFFSET_MASK != 0 {
            return Err(InspectError::Unsupported);
        }
        // These containers are reserved metadata, not arbitrary table records.
        // Public producer/getter bytes establish owner0 for owned usage rows
        // and self-owner1 for the fixed global page1,row0 container.
        let owner = match role {
            MapRole::Owned | MapRole::LongValueCandidate => OWNED_CONTAINER_OWNER,
            #[cfg(feature = "ace16-write-preview")]
            MapRole::TableFreeSpace => OWNED_CONTAINER_OWNER,
            MapRole::GlobalFree => {
                if reference != GLOBAL_FREE_REFERENCE {
                    return Err(InspectError::Unsupported);
                }
                GLOBAL_MAP_PAGE
            }
        };
        if u32_at(&page, OWNER_OFFSET)? != owner {
            return Err(InspectError::Malformed);
        }
        let row = reader.row_location(number, slot, owner, &page)?;
        let result = Self {
            payload: row.data,
            offset: row.offset,
            container: number,
            role,
        };
        match result.payload.first().copied() {
            Some(INLINE_MAP) if result.payload.len() >= INLINE_BITMAP_OFFSET => {}
            Some(INDIRECT_MAP) if (result.payload.len() - 1).is_multiple_of(PAGE_POINTER_BYTES) => {
                for (position, bytes) in result.payload[1..]
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
                    for previous in result.payload[1..1 + position * PAGE_POINTER_BYTES]
                        .as_chunks::<PAGE_POINTER_BYTES>()
                        .0
                    {
                        reader.work(PAGE_POINTER_BYTES)?;
                        if u32_at(previous, 0)? == pointer {
                            return Err(InspectError::Unsupported);
                        }
                    }
                    let bitmap = reader.page(pointer)?;
                    if bitmap[..USAGE_SIGNATURE.len()] != USAGE_SIGNATURE {
                        return Err(InspectError::Malformed);
                    }
                    reader.work(PAGE_BYTES)?;
                }
            }
            _ => return Err(InspectError::Unsupported),
        }
        if !matches!(role, MapRole::GlobalFree) {
            // Owned addresses and qualified table candidate addresses are in
            // file. This shares only bitmap grammar/address validation; their
            // membership meanings remain distinct. Global FREE bits may name
            // future pages and deliberately do not use this validator.
            PageCursor::new(reader, reference)?;
        }
        reader.work(result.payload.len())?;
        Ok(result)
    }

    pub(crate) fn require_allocated<R: NativeReadSource>(
        &self,
        reader: &mut Reader<'_, R>,
        target: u32,
    ) -> Result<(), InspectError> {
        if !matches!(self.role, MapRole::GlobalFree) {
            return Err(InspectError::Unsupported);
        }
        let cell = self.cell(reader, target)?;
        if cell.before & cell.mask != 0 {
            return Err(InspectError::Malformed);
        }
        Ok(())
    }

    pub(crate) fn require_allocated_storage<R: NativeReadSource>(
        &self,
        reader: &mut Reader<'_, R>,
        global: &Self,
    ) -> Result<(), InspectError> {
        if matches!(self.role, MapRole::GlobalFree) {
            // The native header is reserved, and the global container is self-owned.
            global.require_allocated(reader, 0)?;
        }
        global.require_allocated(reader, self.container)?;
        if self.payload[0] == INDIRECT_MAP {
            for bytes in self.payload[1..].as_chunks::<PAGE_POINTER_BYTES>().0 {
                reader.tick()?;
                let pointer = u32_at(bytes, 0)?;
                if pointer != 0 {
                    global.require_allocated(reader, pointer)?;
                }
            }
        }
        Ok(())
    }

    pub(crate) fn cell<R: NativeReadSource>(
        &self,
        reader: &mut Reader<'_, R>,
        target: u32,
    ) -> Result<MapCell, InspectError> {
        reader.tick()?;
        if target >= reader.header.page_count() {
            return Err(InspectError::Malformed);
        }
        self.represented_cell(reader, target)
    }

    /// Locate an already represented cell for a page appended at/after the
    /// captured EOF. This does not allocate bitmap storage or expand a window.
    #[cfg(feature = "ace16-write-preview")]
    pub(crate) fn future_cell<R: NativeReadSource>(
        &self,
        reader: &mut Reader<'_, R>,
        target: u32,
    ) -> Result<MapCell, InspectError> {
        reader.tick()?;
        if target < reader.header.page_count() {
            return Err(InspectError::Malformed);
        }
        self.represented_cell(reader, target)
    }

    fn represented_cell<R: NativeReadSource>(
        &self,
        reader: &mut Reader<'_, R>,
        target: u32,
    ) -> Result<MapCell, InspectError> {
        let (offset, before, bit) = match self.payload[0] {
            INLINE_MAP => {
                let base = u32_at(&self.payload, 1)?;
                let relative = target.checked_sub(base).ok_or(InspectError::Unsupported)? as usize;
                let position = INLINE_BITMAP_OFFSET + relative / u8::BITS as usize;
                let before = *self
                    .payload
                    .get(position)
                    .ok_or(InspectError::Unsupported)?;
                (self.offset + position, before, relative % u8::BITS as usize)
            }
            INDIRECT_MAP => {
                let relative = target as usize % PAGES_PER_BITMAP;
                let slot = target as usize / PAGES_PER_BITMAP;
                let pointer_offset = 1 + slot * PAGE_POINTER_BYTES;
                let bytes = self
                    .payload
                    .get(pointer_offset..pointer_offset + PAGE_POINTER_BYTES)
                    .ok_or(InspectError::Unsupported)?;
                let pointer = u32_at(bytes, 0)?;
                if pointer == 0 {
                    return Err(InspectError::Unsupported);
                }
                let page = reader.page(pointer)?;
                if page[..USAGE_SIGNATURE.len()] != USAGE_SIGNATURE {
                    return Err(InspectError::Malformed);
                }
                let position = INDIRECT_BITMAP_OFFSET + relative / u8::BITS as usize;
                (
                    pointer as usize * PAGE_BYTES + position,
                    page[position],
                    relative % u8::BITS as usize,
                )
            }
            _ => return Err(InspectError::Unsupported),
        };
        Ok(MapCell {
            offset,
            before,
            mask: 1 << bit,
        })
    }
}
