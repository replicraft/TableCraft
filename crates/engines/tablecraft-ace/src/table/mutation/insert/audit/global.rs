//! Source-derived global FREE polarity and one append-frontier promotion.
use super::{copied, reserve};
use crate::{
    EditError, InspectError, NativeImage,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        bytes::u32_at,
        page::{PAGE_BYTES, USAGE_SIGNATURE},
        reader::Reader,
    },
    limits::MAX_IMAGE_BYTES,
    table::mutation::{
        insert::Staged,
        map_authority::{self, Owner, RecordIdentity},
        variable_update::map_audit::{ExpectedByte, expect},
    },
};
use std::io::Cursor;

const GLOBAL_CONTAINER: u32 = 1;
const INLINE: u8 = 0;
const INDIRECT: u8 = 1;
const POINTER_BYTES: usize = size_of::<u32>();
const INLINE_PREFIX: usize = size_of::<u8>() + POINTER_BYTES;
const INLINE_BITMAP_BYTES: usize = 64;
const RECORD_BYTES: usize = INLINE_PREFIX + INLINE_BITMAP_BYTES;
const INLINE_PAGES: u32 = (INLINE_BITMAP_BYTES * u8::BITS as usize) as u32;
// The observed writer passes PAGE_BYTES - header as an exclusive fill end;
// this qualifies an initialization policy, not a reserved format tail.
const QUALIFIED_FREE_END: usize = PAGE_BYTES - USAGE_SIGNATURE.len();
const BITMAP_PAGES: u32 = ((PAGE_BYTES - USAGE_SIGNATURE.len()) * u8::BITS as usize) as u32;

struct Record {
    offset: usize,
    before: Vec<u8>,
    after: Vec<u8>,
}

pub(super) struct Replay<'a> {
    image: &'a NativeImage,
    owner: u32,
    source: Option<RepresentedMap>,
    record: Option<Record>,
    bitmap: Option<u32>,
}

impl<'a> Replay<'a> {
    pub(super) fn new(image: &'a NativeImage, owner: u32) -> Self {
        Self {
            image,
            owner,
            source: None,
            record: None,
            bitmap: None,
        }
    }

    pub(super) fn append(
        &mut self,
        bytes: &mut Vec<u8>,
        edits: &mut Vec<ExpectedByte>,
        zero_columns: &[usize],
        number: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        reader.tick()?;
        if number as usize != bytes.len() / PAGE_BYTES {
            return Err(EditError::Preservation);
        }
        if let Some(bitmap) = self.bitmap {
            if number / BITMAP_PAGES != 0 {
                return Err(InspectError::Unsupported.into());
            }
            let offset = bitmap as usize * PAGE_BYTES
                + USAGE_SIGNATURE.len()
                + number as usize / u8::BITS as usize;
            let mask = 1 << (number % u8::BITS);
            let current = bytes.get_mut(offset).ok_or(EditError::Preservation)?;
            if *current & mask == 0 {
                return Err(EditError::Preservation);
            }
            *current &= !mask;
            return append_page(bytes, number, reader);
        }

        if self.source.is_none() {
            let source = RepresentedMap::read(reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
            source.require_allocated_storage(reader, &source)?;
            self.source = Some(source);
        }
        let source = self.source.as_ref().ok_or(EditError::Preservation)?;
        match source.future_cell(reader, number) {
            Ok(cell) => {
                let offset = cell.offset;
                let current = *bytes.get(offset).ok_or(EditError::Preservation)?;
                if cell.before & cell.mask == 0 || current & cell.mask == 0 {
                    return Err(EditError::Preservation);
                }
                expect(edits, cell, false, reader)?;

                // Several adjacent global bits may share one original byte.
                reader.work(edits.len())?;
                let after = edits
                    .iter()
                    .find(|edit| edit.offset == offset)
                    .ok_or(EditError::Preservation)?
                    .after;
                bytes[offset] = after;
                append_page(bytes, number, reader)
            }
            Err(InspectError::Unsupported) if source.kind() == INLINE => {
                self.promote(bytes, edits, zero_columns, number, reader)
            }
            Err(error) => Err(error.into()),
        }
    }

    fn promote(
        &mut self,
        bytes: &mut Vec<u8>,
        edits: &mut Vec<ExpectedByte>,
        zero_columns: &[usize],
        number: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        if reader.header.page_count() > INLINE_PAGES || number != INLINE_PAGES {
            return Err(InspectError::Unsupported.into());
        }

        let page = reader.page(GLOBAL_CONTAINER)?;
        let row = reader.row_location(GLOBAL_CONTAINER, 0, GLOBAL_CONTAINER, &page)?;
        if row.data.len() != RECORD_BYTES || row.data[0] != INLINE || u32_at(&row.data, 1)? != 0 {
            return Err(InspectError::Unsupported.into());
        }
        let end = row
            .offset
            .checked_add(RECORD_BYTES)
            .ok_or(InspectError::StructureLimit)?;

        let census = map_authority::capture_catalog_for_initialization(
            self.image,
            Some((self.owner, zero_columns)),
            reader.budget,
            reader.cancelled,
            |_, _, _, _| Ok(()),
        )?;
        census.require_record_rewrite(
            &[Owner::GlobalFree],
            RecordIdentity {
                owner: Owner::GlobalFree,
                reference: GLOBAL_FREE_REFERENCE,
                range: row.offset..end,
            },
            reader.budget,
            reader.cancelled,
        )?;

        let bitmap = number.checked_add(1).ok_or(InspectError::StructureLimit)?;
        let before = row.data;
        let mut after = copied(&[0; RECORD_BYTES], reader)?;
        after[0] = INDIRECT;
        after[1..INLINE_PREFIX].copy_from_slice(&bitmap.to_le_bytes());

        reader.work(edits.len())?;
        for edit in edits.iter() {
            reader.tick()?;
            if (row.offset..end).contains(&edit.offset)
                && before[edit.offset - row.offset] != edit.before
            {
                return Err(EditError::Preservation);
            }
        }
        edits.retain(|edit| !(row.offset..end).contains(&edit.offset));
        bytes
            .get_mut(row.offset..end)
            .ok_or(EditError::Preservation)?
            .copy_from_slice(&after);

        append_page(bytes, number, reader)?;
        append_page(bytes, bitmap, reader)?;
        let start = bitmap as usize * PAGE_BYTES;
        bytes[start + USAGE_SIGNATURE.len()..start + QUALIFIED_FREE_END].fill(u8::MAX);
        bytes[start..start + USAGE_SIGNATURE.len()].copy_from_slice(&USAGE_SIGNATURE);
        let full_bytes = (bitmap as usize + 1) / u8::BITS as usize;
        let partial = (bitmap + 1) % u8::BITS;
        let payload = start + USAGE_SIGNATURE.len();
        bytes[payload..payload + full_bytes].fill(0);
        if partial != 0 {
            bytes[payload + full_bytes] &= !((1_u8 << partial) - 1);
        }
        reader.work(RECORD_BYTES + PAGE_BYTES + full_bytes + 1)?;

        self.record = Some(Record {
            offset: row.offset,
            before,
            after,
        });
        self.bitmap = Some(bitmap);
        Ok(())
    }

    pub(super) fn is_metadata(&self, number: u32) -> bool {
        self.bitmap == Some(number)
    }

    pub(super) fn compare(
        &self,
        staged: &Staged,
        bytes: &[u8],
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        let mut purposes = staged.data.append.global_bitmaps();
        if let Some(number) = self.bitmap
            && purposes.next() != Some((number, GLOBAL_FREE_REFERENCE, 0))
        {
            return Err(EditError::Preservation);
        }
        if purposes.next().is_some() {
            return Err(EditError::Preservation);
        }

        match (&self.record, staged.data.append.global_record()) {
            (None, None) => {}
            (Some(expected), Some(actual)) => {
                reader.work(RECORD_BYTES * 2)?;
                if expected.offset != actual.offset
                    || expected.before != actual.before
                    || expected.after != actual.after
                {
                    return Err(EditError::Preservation);
                }
            }
            _ => return Err(EditError::Preservation),
        }
        match (self.bitmap, staged.data.append.global_bitmap()) {
            (None, None) => Ok(()),
            (Some(number), Some(actual)) => {
                let start = number as usize * PAGE_BYTES;
                reader.work(PAGE_BYTES)?;
                if actual.number != number
                    || actual.reference != GLOBAL_FREE_REFERENCE
                    || actual.block != 0
                    || actual.bytes != bytes[start..start + PAGE_BYTES]
                {
                    return Err(EditError::Preservation);
                }
                staged.data.append.require_global_bitmap(
                    number,
                    GLOBAL_FREE_REFERENCE,
                    0,
                    reader.budget,
                    reader.cancelled,
                )
            }
            _ => Err(EditError::Preservation),
        }
    }
}

fn append_page(
    bytes: &mut Vec<u8>,
    number: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    if number as usize != bytes.len() / PAGE_BYTES {
        return Err(EditError::Preservation);
    }
    let length = bytes
        .len()
        .checked_add(PAGE_BYTES)
        .filter(|length| *length as u64 <= MAX_IMAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    reserve(bytes, PAGE_BYTES, reader)?;
    reader.work(PAGE_BYTES)?;
    bytes.resize(length, 0);
    Ok(())
}
