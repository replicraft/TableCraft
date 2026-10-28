//! Bounded existing column hints and explicit new LVAL page reservations.
use super::{Page, Plan, maps::Maps, reserve};
use crate::{
    EditError, InspectError, LongValueMapReferences, NativeImage, TableSchema, WriteBlocker,
    format::{
        allocation::RepresentedMap,
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::Reader,
        row::{MAX_SLOTS, OFFSET_MASK, ROW_COUNT_OFFSET, SLOT_BYTES, reference, slot_offset},
    },
    table::mutation::{
        append::State,
        long_value::{ownership::Inventory, placement::geometry},
    },
};
use std::{io::Cursor, ops::Range};
use tablecraft_core::limits::Resource;

const FREE_OFFSET: usize = DATA_SIGNATURE.len();
const EMPTY_FREE: usize = PAGE_BYTES - slot_offset(0);
const LVAL_OWNER: &[u8] = b"LVAL";
const LINK_BYTES: usize = size_of::<u32>();

pub(super) struct Column {
    pub position: usize,
    pub number: usize,
    pub references: LongValueMapReferences,
    pub maps: Maps,
}

pub(super) struct Builder<'a> {
    pub image: &'a NativeImage,
    pub schema: &'a TableSchema,
    pub inventory: Inventory,
    pub global: RepresentedMap,
    pub columns: Vec<Column>,
    pub plan: Plan,
    pub append: State,
}

pub(super) struct Location {
    pub reference: u32,
    page: usize,
    range: Range<usize>,
}

impl Builder<'_> {
    fn page(
        &mut self,
        column: usize,
        number: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<usize, EditError> {
        reader.work(self.plan.pages.len())?;
        if let Some(position) = self
            .plan
            .pages
            .iter()
            .position(|page| page.number == number)
        {
            if self.plan.pages[position].column != self.columns[column].number {
                return Err(EditError::Preservation);
            }
            return Ok(position);
        }
        let start = (number as usize)
            .checked_mul(PAGE_BYTES)
            .ok_or(InspectError::StructureLimit)?;
        let raw = self
            .image
            .as_bytes()
            .get(start..start + PAGE_BYTES)
            .ok_or(InspectError::Truncated)?;
        if !raw.starts_with(&DATA_SIGNATURE)
            || raw.get(OWNER_OFFSET..OWNER_OFFSET + LVAL_OWNER.len()) != Some(LVAL_OWNER)
        {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        geometry(raw)?;
        self.push(column, number, raw, raw, reader)
    }

    fn push(
        &mut self,
        column: usize,
        number: u32,
        before: &[u8],
        after: &[u8],
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<usize, EditError> {
        reserve(&mut self.plan.pages, 1, reader.budget)?;
        let before = copied(before, reader)?;
        let after = copied(after, reader)?;
        let references = self.columns[column].references;
        let position = self.plan.pages.len();
        self.plan.pages.push(Page {
            number,
            column: self.columns[column].number,
            owned: references.owned(),
            candidate: references.candidate(),
            before,
            after,
        });
        Ok(position)
    }

    fn destination(
        &mut self,
        column: usize,
        length: usize,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<usize, EditError> {
        if length
            .checked_add(SLOT_BYTES)
            .is_none_or(|length| length > EMPTY_FREE)
        {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        // LVAL retains its existing bounded retry policy, independently of the
        // ordinary DATA allocator's single highest-candidate attempt.
        while let Some(number) = self.columns[column].maps.highest(reader)? {
            let position = self.page(column, number, reader)?;
            let (count, end) = geometry(&self.plan.pages[position].after)?;
            if count < MAX_SLOTS && length + SLOT_BYTES <= end - slot_offset(count) {
                return Ok(position);
            }
            self.columns[column].maps.remove_hint(
                number,
                self.schema,
                &self.inventory,
                &mut self.append,
                reader,
            )?;
        }
        let number = self.columns[column].maps.allocate(
            self.schema,
            &self.inventory,
            &self.global,
            &mut self.append,
            &mut self.plan.metadata,
            reader,
        )?;
        let mut initial = [0; PAGE_BYTES];
        initial[..DATA_SIGNATURE.len()].copy_from_slice(&DATA_SIGNATURE);
        initial[FREE_OFFSET..FREE_OFFSET + SLOT_BYTES]
            .copy_from_slice(&(EMPTY_FREE as u16).to_le_bytes());
        initial[OWNER_OFFSET..OWNER_OFFSET + LVAL_OWNER.len()].copy_from_slice(LVAL_OWNER);
        self.push(column, number, &[], &initial, reader)
    }

    pub(super) fn append(
        &mut self,
        selected: usize,
        bytes: &[u8],
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Location, EditError> {
        reader.tick()?;
        reader
            .budget
            .charge(Resource::Rows, 1)
            .map_err(InspectError::from)?;
        reader.work(self.columns.len())?;
        let column = self
            .columns
            .iter()
            .position(|column| column.position == selected)
            .ok_or(EditError::Preservation)?;
        let position = self.destination(column, bytes.len(), reader)?;
        let page = &mut self.plan.pages[position];
        let (count, end) = geometry(&page.after)?;
        let start = end
            .checked_sub(bytes.len())
            .filter(|start| *start >= slot_offset(count + 1))
            .ok_or(EditError::Preservation)?;
        let word = u16::try_from(start).map_err(|_| InspectError::StructureLimit)?;
        if count >= MAX_SLOTS || word & !OFFSET_MASK != 0 {
            return Err(EditError::Preservation);
        }
        reader.work(bytes.len() + 3 * SLOT_BYTES)?;
        page.after[start..end].copy_from_slice(bytes);
        page.after[slot_offset(count)..slot_offset(count + 1)].copy_from_slice(&word.to_le_bytes());
        page.after[ROW_COUNT_OFFSET..ROW_COUNT_OFFSET + SLOT_BYTES]
            .copy_from_slice(&((count + 1) as u16).to_le_bytes());
        page.after[FREE_OFFSET..FREE_OFFSET + SLOT_BYTES]
            .copy_from_slice(&((start - slot_offset(count + 1)) as u16).to_le_bytes());
        Ok(Location {
            reference: reference(page.number, count as u8),
            page: position,
            range: start..end,
        })
    }

    pub(super) fn link(
        &mut self,
        location: &Location,
        next: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        reader.tick()?;
        if location.range.len() < LINK_BYTES {
            return Err(EditError::Preservation);
        }
        reader.work(LINK_BYTES)?;
        self.plan
            .pages
            .get_mut(location.page)
            .ok_or(EditError::Preservation)?
            .after[location.range.start..location.range.start + LINK_BYTES]
            .copy_from_slice(&next.to_le_bytes());
        Ok(())
    }
}

fn copied(bytes: &[u8], reader: &mut Reader<'_, Cursor<&[u8]>>) -> Result<Vec<u8>, EditError> {
    let mut result = Vec::new();
    reserve(&mut result, bytes.len(), reader.budget)?;
    reader.work(bytes.len())?;
    result.extend_from_slice(bytes);
    Ok(result)
}
