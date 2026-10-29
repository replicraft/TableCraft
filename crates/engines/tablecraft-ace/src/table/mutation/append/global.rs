//! INSERT-only canonical global promotion; original map cells remain immutable.
#[cfg(test)]
mod tests;
use super::Edit;
use crate::{
    EditError, InspectError, WriteBlocker,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        bytes::u32_at,
        page::{PAGE_BYTES, USAGE_SIGNATURE},
        reader::Reader,
        row::{reference_page, reference_slot},
    },
    table::mutation::map_authority::{Owner, RecordIdentity},
};
use std::io::Cursor;

const INLINE: u8 = 0;
const INDIRECT: u8 = 1;
const INLINE_PREFIX: usize = 1 + size_of::<u32>();
const INLINE_BYTES: usize = INLINE_PREFIX + 64;
pub(super) const INLINE_PAGES: u32 = 512;
const BITMAP_PREFIX: usize = USAGE_SIGNATURE.len();
const BITMAP_PAGES: u32 = ((PAGE_BYTES - BITMAP_PREFIX) * u8::BITS as usize) as u32;
// Qualified Jackcess initialization passes PAGE_BYTES - dataStart as the
// exclusive fill endpoint; the final physical payload bytes remain zero.
const QUALIFIED_INITIAL_FILL_END: usize = PAGE_BYTES - BITMAP_PREFIX;
const GLOBAL_OWNER: u32 = 1;

pub(in crate::table) struct GlobalRecord {
    pub offset: usize,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}

pub(in crate::table) struct GlobalBitmap {
    pub number: u32,
    pub reference: u32,
    pub block: u32,
    pub bytes: Vec<u8>,
}

pub(super) struct Growth {
    offset: usize,
    before: Vec<u8>,
    record: Option<GlobalRecord>,
    bitmap: Option<GlobalBitmap>,
}

impl Growth {
    pub(super) fn capture(
        reader: &mut Reader<'_, Cursor<&[u8]>>,
        certify: impl FnOnce(RecordIdentity, &mut Reader<'_, Cursor<&[u8]>>) -> Result<(), EditError>,
    ) -> Result<Option<Self>, EditError> {
        if reader.header.page_count() > INLINE_PAGES {
            return Ok(None);
        }
        let global = RepresentedMap::read(reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
        if global.kind() != INLINE {
            return Ok(None);
        }
        let page = reference_page(GLOBAL_FREE_REFERENCE);
        let raw = reader.page(page)?;
        let row = reader.row_location(
            page,
            reference_slot(GLOBAL_FREE_REFERENCE),
            GLOBAL_OWNER,
            &raw,
        )?;
        if row.data.len() != INLINE_BYTES || u32_at(&row.data, 1)? != 0 {
            return Ok(None);
        }
        let identity = RecordIdentity {
            owner: Owner::GlobalFree,
            reference: GLOBAL_FREE_REFERENCE,
            range: row.offset..row.offset + row.data.len(),
        };
        match certify(identity, reader) {
            Ok(()) => {}
            Err(EditError::Unsupported(WriteBlocker::RowLayout)) => return Ok(None),
            Err(error) => return Err(error),
        }
        Ok(Some(Self {
            offset: row.offset,
            before: row.data,
            record: None,
            bitmap: None,
        }))
    }

    pub(super) fn record(&self) -> Option<&GlobalRecord> {
        self.record.as_ref()
    }

    pub(super) fn bitmap(&self) -> Option<&GlobalBitmap> {
        self.bitmap.as_ref()
    }

    #[cfg(test)]
    pub(super) fn record_mut(&mut self) -> Option<&mut GlobalRecord> {
        self.record.as_mut()
    }

    #[cfg(test)]
    pub(super) fn bitmap_mut(&mut self) -> Option<&mut GlobalBitmap> {
        self.bitmap.as_mut()
    }

    pub(super) fn is_free(
        &self,
        number: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<bool, EditError> {
        reader.tick()?;
        if number >= BITMAP_PAGES {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let Some(bitmap) = &self.bitmap else {
            return Ok(number >= INLINE_PAGES);
        };
        let at = BITMAP_PREFIX + number as usize / u8::BITS as usize;
        Ok(bitmap.bytes[at] & (1 << (number % u8::BITS)) != 0)
    }

    pub(super) fn claim(
        &mut self,
        number: u32,
        edits: &mut Vec<Edit>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        let promoted = self.bitmap.is_none();
        if promoted {
            self.promote(number, edits, reader)?;
        }
        let bitmap = self.bitmap.as_mut().ok_or(EditError::Preservation)?;
        if number >= BITMAP_PAGES || number == bitmap.number {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        let at = BITMAP_PREFIX + number as usize / u8::BITS as usize;
        let mask = 1 << (number % u8::BITS);
        reader.tick()?;
        // The first requested page is already covered by the used frontier.
        if promoted {
            return Ok(());
        }
        if bitmap.bytes[at] & mask == 0 {
            return Err(EditError::Preservation);
        }
        bitmap.bytes[at] &= !mask;
        Ok(())
    }

    fn promote(
        &mut self,
        number: u32,
        edits: &mut Vec<Edit>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        let metadata = number.checked_add(1).ok_or(InspectError::StructureLimit)?;
        if number != INLINE_PAGES || metadata >= BITMAP_PAGES {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        reader.work(edits.len())?;
        let end = self.offset + self.before.len();
        for edit in edits.iter() {
            reader.tick()?;
            if (self.offset..end).contains(&edit.offset)
                && self.before[edit.offset - self.offset] != edit.before
            {
                return Err(EditError::Preservation);
            }
        }
        let mut after = Vec::new();
        reader
            .budget
            .charge_allocation(INLINE_BYTES as u64, 1)
            .map_err(InspectError::from)?;
        after
            .try_reserve_exact(INLINE_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        reader.work(INLINE_BYTES)?;
        after.resize(INLINE_BYTES, 0);
        after[0] = INDIRECT;
        after[1..INLINE_PREFIX].copy_from_slice(&metadata.to_le_bytes());
        let mut bytes = Vec::new();
        reader
            .budget
            .charge_allocation(PAGE_BYTES as u64, 1)
            .map_err(InspectError::from)?;
        bytes
            .try_reserve_exact(PAGE_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        reader.work(PAGE_BYTES + QUALIFIED_INITIAL_FILL_END - BITMAP_PREFIX)?;
        bytes.resize(PAGE_BYTES, 0);
        bytes[BITMAP_PREFIX..QUALIFIED_INITIAL_FILL_END].fill(u8::MAX);
        bytes[..BITMAP_PREFIX].copy_from_slice(&USAGE_SIGNATURE);
        for page in 0..=metadata {
            reader.tick()?;
            bytes[BITMAP_PREFIX + page as usize / u8::BITS as usize] &= !(1 << (page % u8::BITS));
        }
        let mut before = Vec::new();
        reader
            .budget
            .charge_allocation(INLINE_BYTES as u64, 1)
            .map_err(InspectError::from)?;
        before
            .try_reserve_exact(INLINE_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        reader.work(INLINE_BYTES + edits.len())?;
        before.extend_from_slice(&self.before);
        edits.retain(|edit| !(self.offset..end).contains(&edit.offset));
        self.record = Some(GlobalRecord {
            offset: self.offset,
            before,
            after,
        });
        self.bitmap = Some(GlobalBitmap {
            number: metadata,
            reference: GLOBAL_FREE_REFERENCE,
            block: 0,
            bytes,
        });
        Ok(())
    }
}
