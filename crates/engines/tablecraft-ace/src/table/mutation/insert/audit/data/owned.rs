//! SOURCE-authorized ordinary-owned window replay, independent of construction.
use super::{ImageProof, State, reserve};
use crate::{
    EditError, InspectError,
    format::{
        allocation::RepresentedMap,
        bytes::u32_at,
        page::{PAGE_BYTES, USAGE_SIGNATURE},
        reader::Reader,
        row::{reference_page, reference_slot},
    },
    table::mutation::{
        map_authority::Owner,
        variable_split::metadata::Metadata,
        variable_update::map_audit::{ExpectedByte, expect},
    },
};
use std::io::Cursor;

const INLINE: u8 = 0;
const POINTER_BYTES: usize = size_of::<u32>();
const PREFIX_BYTES: usize = size_of::<u8>() + POINTER_BYTES;
const BITMAP_BYTES: usize = 64;
const RECORD_BYTES: usize = PREFIX_BYTES + BITMAP_BYTES;
const WINDOW_PAGES: u32 = (BITMAP_BYTES * u8::BITS as usize) as u32;
const INDIRECT: u8 = 1;
const BITMAP_PREFIX: usize = USAGE_SIGNATURE.len();
const BITMAP_PAGES: u32 = ((PAGE_BYTES - BITMAP_PREFIX) * u8::BITS as usize) as u32;

pub(super) struct ExpectedBitmap {
    pub number: u32,
    pub reference: u32,
    pub block: u32,
    pub bytes: Vec<u8>,
}

struct Record {
    offset: usize,
    before: Vec<u8>,
    base: u32,
}

pub(super) struct Replay<'a> {
    original: &'a RepresentedMap,
    definition: u32,
    reference: u32,
    record: Option<Record>,
    bitmap: Option<ExpectedBitmap>,
}

impl<'a> Replay<'a> {
    pub(super) fn new(original: &'a RepresentedMap, definition: u32, reference: u32) -> Self {
        Self {
            original,
            definition,
            reference,
            record: None,
            bitmap: None,
        }
    }

    pub(super) fn add(
        &mut self,
        number: u32,
        states: &[State],
        metadata: &Metadata,
        proof: &mut ImageProof<'_>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        if let Some(bitmap) = &mut self.bitmap {
            add_member(&mut bitmap.bytes, number, bitmap.number, reader)?;
            let begin = bitmap.number as usize * PAGE_BYTES;
            reader.work(PAGE_BYTES)?;
            proof.bytes[begin..begin + PAGE_BYTES].copy_from_slice(&bitmap.bytes);
            return Ok(());
        }
        if self.record.is_none() {
            match self.original.future_cell(reader, number) {
                Ok(cell) => {
                    if cell.before & cell.mask != 0 {
                        return Err(EditError::Preservation);
                    }
                    expect(&mut proof.map_edits, cell, true, reader)?;
                    return Ok(());
                }
                Err(InspectError::Unsupported) if self.original.kind() == INLINE => {}
                Err(error) => return Err(error.into()),
            }

            let page_number = reference_page(self.reference);
            let page = reader.page(page_number)?;
            let row = reader.row_location(page_number, reference_slot(self.reference), 0, &page)?;
            if row.data.len() != RECORD_BYTES || row.data[0] != INLINE {
                return Err(InspectError::Unsupported.into());
            }
            let end = row
                .offset
                .checked_add(RECORD_BYTES)
                .ok_or(InspectError::StructureLimit)?;
            metadata.require_record_rewrite(
                Owner::OrdinaryOwned {
                    definition: self.definition,
                },
                self.reference,
                row.offset..end,
                reader.budget,
                reader.cancelled,
            )?;
            self.record = Some(Record {
                offset: row.offset,
                base: u32_at(&row.data, 1)?,
                before: row.data,
            });
        }

        let record = self.record.as_mut().ok_or(EditError::Preservation)?;
        let bounds_work = states
            .len()
            .checked_mul(2)
            .ok_or(InspectError::StructureLimit)?;
        reader.work(bounds_work)?;
        let minimum = states
            .iter()
            .map(|state| state.number)
            .chain([number])
            .min()
            .ok_or(EditError::Preservation)?;
        let maximum = states
            .iter()
            .map(|state| state.number)
            .chain([number])
            .max()
            .ok_or(EditError::Preservation)?;
        let base = if number
            .checked_sub(record.base)
            .is_some_and(|distance| distance < WINDOW_PAGES)
        {
            record.base
        } else {
            minimum - minimum % u8::BITS
        };
        let fits = maximum
            .checked_sub(base)
            .is_some_and(|distance| distance < WINDOW_PAGES);
        let mut after = Vec::new();
        reserve(&mut after, RECORD_BYTES, reader)?;
        reader.work(RECORD_BYTES)?;
        after.resize(RECORD_BYTES, 0);
        if fits {
            after[1..PREFIX_BYTES].copy_from_slice(&base.to_le_bytes());
            for member in states.iter().map(|state| state.number).chain([number]) {
                reader.tick()?;
                let relative = member
                    .checked_sub(base)
                    .filter(|distance| *distance < WINDOW_PAGES)
                    .ok_or(EditError::Preservation)?;
                let at = PREFIX_BYTES + relative as usize / u8::BITS as usize;
                let mask = 1 << (relative % u8::BITS);
                if after[at] & mask != 0 {
                    return Err(EditError::Preservation);
                }
                after[at] |= mask;
            }
        } else {
            let metadata = u32::try_from(proof.bytes.len() / PAGE_BYTES)
                .map_err(|_| InspectError::StructureLimit)?;
            let mut bytes = Vec::new();
            reserve(&mut bytes, PAGE_BYTES, reader)?;
            reader.work(PAGE_BYTES)?;
            bytes.resize(PAGE_BYTES, 0);
            bytes[..BITMAP_PREFIX].copy_from_slice(&USAGE_SIGNATURE);
            for member in states.iter().map(|state| state.number).chain([number]) {
                add_member(&mut bytes, member, metadata, reader)?;
            }
            proof.append(metadata, reader)?;
            let begin = metadata as usize * PAGE_BYTES;
            reader.work(PAGE_BYTES)?;
            proof.bytes[begin..begin + PAGE_BYTES].copy_from_slice(&bytes);
            after[0] = INDIRECT;
            after[1..PREFIX_BYTES].copy_from_slice(&metadata.to_le_bytes());
            self.bitmap = Some(ExpectedBitmap {
                number: metadata,
                reference: self.reference,
                block: 0,
                bytes,
            });
        }

        let end = record
            .offset
            .checked_add(RECORD_BYTES)
            .ok_or(InspectError::StructureLimit)?;
        let work = proof
            .map_edits
            .len()
            .checked_mul(2)
            .ok_or(InspectError::StructureLimit)?;
        reader.work(work)?;
        for edit in &proof.map_edits {
            reader.tick()?;
            if (record.offset..end).contains(&edit.offset)
                && record.before[edit.offset - record.offset] != edit.before
            {
                return Err(EditError::Preservation);
            }
        }
        reserve(&mut proof.map_edits, RECORD_BYTES, reader)?;
        proof
            .map_edits
            .retain(|edit| !(record.offset..end).contains(&edit.offset));
        for (position, (&before, &after)) in record.before.iter().zip(&after).enumerate() {
            reader.tick()?;
            proof.map_edits.push(ExpectedByte {
                offset: record.offset + position,
                before,
                after,
            });
        }
        reader.work(RECORD_BYTES)?;
        proof.bytes[record.offset..end].copy_from_slice(&after);
        record.base = base;
        Ok(())
    }

    pub(super) fn into_bitmap(self) -> Option<ExpectedBitmap> {
        self.bitmap
    }
}

fn add_member(
    bytes: &mut [u8],
    member: u32,
    metadata: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.tick()?;
    if member >= BITMAP_PAGES || member == metadata {
        return Err(EditError::Preservation);
    }
    let at = BITMAP_PREFIX + member as usize / u8::BITS as usize;
    let mask = 1 << (member % u8::BITS);
    if bytes[at] & mask != 0 {
        return Err(EditError::Preservation);
    }
    bytes[at] |= mask;
    Ok(())
}
