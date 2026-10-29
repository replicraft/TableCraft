//! SOURCE-authorized ordinary-owned window replay, independent of construction.
use super::{ImageProof, State, reserve};
use crate::{
    EditError, InspectError,
    format::{
        allocation::RepresentedMap,
        bytes::u32_at,
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
}

impl<'a> Replay<'a> {
    pub(super) fn new(original: &'a RepresentedMap, definition: u32, reference: u32) -> Self {
        Self {
            original,
            definition,
            reference,
            record: None,
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
            let aligned = minimum - minimum % u8::BITS;
            if maximum
                .checked_sub(aligned)
                .is_none_or(|distance| distance >= WINDOW_PAGES)
            {
                return Err(InspectError::Unsupported.into());
            }
            aligned
        };
        let mut after = Vec::new();
        reserve(&mut after, RECORD_BYTES, reader)?;
        reader.work(RECORD_BYTES)?;
        after.resize(RECORD_BYTES, 0);
        after[1..PREFIX_BYTES].copy_from_slice(&base.to_le_bytes());
        for member in states.iter().map(|state| state.number).chain([number]) {
            reader.tick()?;
            let relative = member
                .checked_sub(base)
                .filter(|distance| *distance < WINDOW_PAGES)
                .ok_or(EditError::Preservation)?;
            let offset = PREFIX_BYTES + relative as usize / u8::BITS as usize;
            let mask = 1 << (relative % u8::BITS);
            if after[offset] & mask != 0 {
                return Err(EditError::Preservation);
            }
            after[offset] |= mask;
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
}
