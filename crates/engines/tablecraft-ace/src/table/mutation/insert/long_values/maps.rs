//! Canonical column rows grow without changing their stored declaration refs.
#[cfg(test)]
mod tests;

use super::{BitmapPage, reserve};
use crate::{
    EditError, InspectError, LongValueMapReferences, TableSchema, WriteBlocker,
    format::{
        allocation::{self, MapCell, RepresentedMap},
        bytes::u32_at,
        page::{DATA_SIGNATURE, PAGE_BYTES, USAGE_SIGNATURE},
        reader::Reader,
        row::{ROW_COUNT_OFFSET, SLOT_BYTES, reference_page, reference_slot, slot_offset},
    },
    table::mutation::{
        append::{self, State},
        long_value::{
            maps::Scope,
            ownership::{ColumnMapKind, Inventory},
        },
    },
};
use std::io::Cursor;

const MAP_RECORDS: usize = 2;
const OWNED_SLOT: usize = 0;
const CANDIDATE_SLOT: usize = 1;
const FREE_OFFSET: usize = DATA_SIGNATURE.len();
const INLINE: u8 = 0;
const INDIRECT: u8 = 1;
const KIND_BYTES: usize = size_of::<u8>();
const POINTER_BYTES: usize = size_of::<u32>();
const INLINE_PREFIX: usize = KIND_BYTES + POINTER_BYTES;
const INLINE_BITMAP_BYTES: usize = 64;
const INLINE_RECORD_BYTES: usize = INLINE_PREFIX + INLINE_BITMAP_BYTES;
const INLINE_PAGES: u32 = (INLINE_BITMAP_BYTES * u8::BITS as usize) as u32;
const BITMAP_PREFIX: usize = USAGE_SIGNATURE.len();
const BITMAP_PAGES: u32 = ((PAGE_BYTES - BITMAP_PREFIX) * u8::BITS as usize) as u32;

struct Record {
    reference: u32,
    column: usize,
    kind: ColumnMapKind,
    offset: usize,
    before: Vec<u8>,
    after: Option<Vec<u8>>,
    fresh: bool,
}

impl Record {
    fn capture(
        reference: u32,
        column: usize,
        kind: ColumnMapKind,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Self, EditError> {
        let number = reference_page(reference);
        let page = reader.page(number)?;
        let row = reader.row_location(number, reference_slot(reference), 0, &page)?;
        Ok(Self {
            reference,
            column,
            kind,
            offset: row.offset,
            before: row.data,
            after: None,
            fresh: false,
        })
    }

    fn current(&self) -> &[u8] {
        self.after.as_deref().unwrap_or(&self.before)
    }

    fn rewrite(
        &mut self,
        after: Vec<u8>,
        schema: &TableSchema,
        inventory: &Inventory,
        append: &mut State,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        if self.fresh {
            if after.len() != self.before.len() {
                return Err(EditError::Preservation);
            }
            self.after = Some(after);
            return Ok(());
        }
        let end = self
            .offset
            .checked_add(self.before.len())
            .ok_or(EditError::Preservation)?;
        inventory.require_record_rewrite(
            schema,
            self.column,
            self.kind,
            (self.reference, self.offset..end),
            reader.budget,
            reader.cancelled,
        )?;
        append::rewrite_record(
            &mut append.map_edits,
            self.offset,
            &self.before,
            &after,
            reader,
        )?;
        self.after = Some(after);
        Ok(())
    }
}

pub(super) struct Maps {
    scope: Option<Scope>,
    pages: Vec<u32>,
    column: usize,
    references: LongValueMapReferences,
    owned: Record,
    candidate: Record,
    hints: Vec<u32>,
}

impl Maps {
    pub(super) fn capture(
        scope: Scope,
        column: usize,
        references: LongValueMapReferences,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Self, EditError> {
        let hints = allocation::pages(reader, references.candidate())?;
        let pages = copied_members(&scope.pages, reader)?;
        Ok(Self {
            scope: Some(scope),
            pages,
            column,
            references,
            owned: Record::capture(references.owned(), column, ColumnMapKind::Owned, reader)?,
            candidate: Record::capture(
                references.candidate(),
                column,
                ColumnMapKind::Candidate,
                reader,
            )?,
            hints,
        })
    }

    pub(super) fn fresh(
        column: usize,
        references: LongValueMapReferences,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Self, EditError> {
        let mut record = |reference, kind| -> Result<Record, EditError> {
            let before = zeros(INLINE_RECORD_BYTES, reader)?;
            let after = Some(zeros(INLINE_RECORD_BYTES, reader)?);
            Ok(Record {
                reference,
                column,
                kind,
                offset: 0,
                before,
                after,
                fresh: true,
            })
        };
        Ok(Self {
            scope: None,
            pages: Vec::new(),
            column,
            references,
            owned: record(references.owned(), ColumnMapKind::Owned)?,
            candidate: record(references.candidate(), ColumnMapKind::Candidate)?,
            hints: Vec::new(),
        })
    }

    pub(super) fn initialized_bytes(
        &self,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Vec<u8>, EditError> {
        if self.scope.is_some() || !self.owned.fresh || !self.candidate.fresh {
            return Err(EditError::Preservation);
        }
        let mut bytes = zeros(PAGE_BYTES, reader)?;
        let first = PAGE_BYTES - INLINE_RECORD_BYTES;
        let second = first - INLINE_RECORD_BYTES;
        bytes[..DATA_SIGNATURE.len()].copy_from_slice(&DATA_SIGNATURE);
        bytes[FREE_OFFSET..FREE_OFFSET + SLOT_BYTES]
            .copy_from_slice(&((second - slot_offset(MAP_RECORDS)) as u16).to_le_bytes());
        bytes[ROW_COUNT_OFFSET..ROW_COUNT_OFFSET + SLOT_BYTES]
            .copy_from_slice(&(MAP_RECORDS as u16).to_le_bytes());
        bytes[slot_offset(OWNED_SLOT)..slot_offset(OWNED_SLOT) + SLOT_BYTES]
            .copy_from_slice(&(first as u16).to_le_bytes());
        bytes[slot_offset(CANDIDATE_SLOT)..slot_offset(CANDIDATE_SLOT) + SLOT_BYTES]
            .copy_from_slice(&(second as u16).to_le_bytes());
        bytes[first..].copy_from_slice(self.owned.current());
        bytes[second..first].copy_from_slice(self.candidate.current());
        Ok(bytes)
    }

    pub(super) fn highest(
        &self,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Option<u32>, EditError> {
        reader.tick()?;
        Ok(self.hints.last().copied())
    }

    pub(super) fn remove_hint(
        &mut self,
        number: u32,
        schema: &TableSchema,
        inventory: &Inventory,
        append: &mut State,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        if self.hints.pop() != Some(number) {
            return Err(EditError::Preservation);
        }
        if self.candidate.after.is_none() {
            let cell = source_cell(
                &self.scope.as_ref().ok_or(EditError::Preservation)?.hint,
                append,
                number,
                reader,
            )?;
            append::clear_hint(&mut append.map_edits, cell, reader)?;
        } else {
            let base = u32_at(self.candidate.current(), KIND_BYTES)?;
            let after = inline(base, &self.hints, reader)?;
            self.candidate
                .rewrite(after, schema, inventory, append, reader)?;
        }
        Ok(())
    }

    /// Return the next body page, then append any required bitmap pages.
    pub(super) fn allocate(
        &mut self,
        schema: &TableSchema,
        inventory: &Inventory,
        global: &RepresentedMap,
        append: &mut State,
        metadata: &mut Vec<BitmapPage>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<u32, EditError> {
        reader.tick()?;
        let column = self.column;
        let references = self.references;
        if !self.hints.is_empty() {
            return Err(EditError::Preservation);
        }
        let number = append.next()?;
        reader.work(self.pages.len())?;
        if self.pages.binary_search(&number).is_ok() {
            return Err(EditError::Preservation);
        }
        let mut members = copied_members(&self.pages, reader)?;
        reserve(&mut members, 1, reader.budget)?;
        members.push(number);
        let mut blocks = Vec::new();
        let mut replacement = None;
        let mut represented = None;
        let current = self.owned.current();
        match current.first().copied() {
            Some(INLINE) => {
                if self.owned.after.is_none() {
                    match self
                        .scope
                        .as_ref()
                        .ok_or(EditError::Preservation)?
                        .owned
                        .future_cell(reader, number)
                    {
                        Ok(cell) => represented = Some(cell),
                        Err(InspectError::Unsupported) => {}
                        Err(error) => return Err(error.into()),
                    }
                }
                if represented.is_none() {
                    if current.len() != INLINE_RECORD_BYTES {
                        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                    }
                    let base = u32_at(current, KIND_BYTES)?;
                    let first = *members.first().ok_or(EditError::Preservation)?;
                    let aligned = first - first % u8::BITS;
                    let represented_now = number
                        .checked_sub(base)
                        .is_some_and(|relative| relative < INLINE_PAGES);
                    let span = u64::from(number) - u64::from(aligned) + 1;
                    if represented_now || span <= u64::from(INLINE_PAGES) {
                        replacement = Some(inline(
                            if represented_now { base } else { aligned },
                            &members,
                            reader,
                        )?);
                    } else {
                        let pointer_count = (current.len() - KIND_BYTES) / POINTER_BYTES;
                        for &member in &members {
                            reader.tick()?;
                            let block = member / BITMAP_PAGES;
                            if block as usize >= pointer_count {
                                return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                            }
                            if blocks.last() != Some(&block) {
                                reserve(&mut blocks, 1, reader.budget)?;
                                blocks.push(block);
                            }
                        }
                        let mut after = zeros(current.len(), reader)?;
                        after[0] = INDIRECT;
                        for (ordinal, &block) in blocks.iter().enumerate() {
                            reader.tick()?;
                            let bitmap = metadata_number(number, ordinal)?;
                            let at = KIND_BYTES + block as usize * POINTER_BYTES;
                            after[at..at + POINTER_BYTES].copy_from_slice(&bitmap.to_le_bytes());
                        }
                        replacement = Some(after);
                    }
                }
            }
            Some(INDIRECT) => {
                let block = number / BITMAP_PAGES;
                let at = KIND_BYTES
                    .checked_add(block as usize * POINTER_BYTES)
                    .ok_or(InspectError::StructureLimit)?;
                let pointer = current
                    .get(at..at + POINTER_BYTES)
                    .ok_or(EditError::Unsupported(WriteBlocker::RowLayout))?;
                let pointer =
                    u32::from_le_bytes(pointer.try_into().map_err(|_| EditError::Preservation)?);
                if pointer == 0 {
                    reserve(&mut blocks, 1, reader.budget)?;
                    blocks.push(block);
                    let mut after = copied(current, reader)?;
                    after[at..at + POINTER_BYTES]
                        .copy_from_slice(&metadata_number(number, 0)?.to_le_bytes());
                    replacement = Some(after);
                } else {
                    reader.work(metadata.len())?;
                    if let Some(bitmap) =
                        metadata.iter_mut().find(|bitmap| bitmap.number == pointer)
                    {
                        if bitmap.column != column
                            || bitmap.owned != references.owned()
                            || bitmap.block != block
                        {
                            return Err(EditError::Preservation);
                        }
                        set_bitmap(&mut bitmap.bytes, number, block, reader)?;
                    } else {
                        represented = Some(
                            self.scope
                                .as_ref()
                                .ok_or(EditError::Preservation)?
                                .owned
                                .future_cell(reader, number)?,
                        );
                    }
                }
            }
            _ => return Err(EditError::Unsupported(WriteBlocker::RowLayout)),
        }

        // Every reservation uses the original global cell and its shared overlay.
        // Metadata pages are map storage, never members of the LVAL owned set.
        for ordinal in 0..=blocks.len() {
            reader.tick()?;
            let page = number
                .checked_add(ordinal as u32)
                .ok_or(InspectError::StructureLimit)?;
            let cell = global.future_cell(reader, page)?;
            if !append::current(&append.map_edits, &cell, reader)? {
                return Err(EditError::Preservation);
            }
        }
        if blocks.is_empty() {
            append.reserve_long_value(
                number,
                column,
                references.owned(),
                references.candidate(),
                reader.budget,
                reader.cancelled,
            )?;
        } else {
            append.reserve_long_value_with_owned_bitmaps(
                number,
                column,
                references,
                &blocks,
                reader.budget,
                reader.cancelled,
            )?;
        }
        for ordinal in 0..=blocks.len() {
            reader.tick()?;
            let page = number
                .checked_add(ordinal as u32)
                .ok_or(InspectError::StructureLimit)?;
            append::claim(
                &mut append.map_edits,
                global.future_cell(reader, page)?,
                false,
                reader,
            )?;
        }
        if let Some(cell) = represented {
            if append::current(&append.map_edits, &cell, reader)? {
                return Err(EditError::Preservation);
            }
            append::claim(&mut append.map_edits, cell, true, reader)?;
        }
        if let Some(after) = replacement {
            self.owned
                .rewrite(after, schema, inventory, append, reader)?;
        }
        reserve(metadata, blocks.len(), reader.budget)?;
        for (ordinal, &block) in blocks.iter().enumerate() {
            let mut bytes = zeros(PAGE_BYTES, reader)?;
            bytes[..BITMAP_PREFIX].copy_from_slice(&USAGE_SIGNATURE);
            for &member in &members {
                reader.tick()?;
                if member / BITMAP_PAGES == block {
                    set_bitmap(&mut bytes, member, block, reader)?;
                }
            }
            metadata.push(BitmapPage {
                number: metadata_number(number, ordinal)?,
                column,
                owned: references.owned(),
                block,
                bytes,
            });
        }

        if self.candidate.after.is_none() {
            match self
                .scope
                .as_ref()
                .ok_or(EditError::Preservation)?
                .hint
                .future_cell(reader, number)
            {
                Ok(cell) => {
                    if append::current(&append.map_edits, &cell, reader)? {
                        return Err(EditError::Preservation);
                    }
                    append::claim(&mut append.map_edits, cell, true, reader)?;
                }
                Err(InspectError::Unsupported) => {
                    if self.candidate.current().len() != INLINE_RECORD_BYTES {
                        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
                    }
                    let base = number - number % u8::BITS;
                    let after = inline(base, &[number], reader)?;
                    self.candidate
                        .rewrite(after, schema, inventory, append, reader)?;
                }
                Err(error) => return Err(error.into()),
            }
        } else {
            let current = u32_at(self.candidate.current(), KIND_BYTES)?;
            let base = if number
                .checked_sub(current)
                .is_some_and(|relative| relative < INLINE_PAGES)
            {
                current
            } else {
                number - number % u8::BITS
            };
            let after = inline(base, &[number], reader)?;
            self.candidate
                .rewrite(after, schema, inventory, append, reader)?;
        }
        reserve(&mut self.hints, 1, reader.budget)?;
        self.hints.push(number);
        self.pages = members;
        Ok(number)
    }
}

fn source_cell(
    map: &RepresentedMap,
    append: &State,
    number: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<MapCell, EditError> {
    Ok(if number as usize >= append.original_length / PAGE_BYTES {
        map.future_cell(reader, number)?
    } else {
        map.cell(reader, number)?
    })
}

fn metadata_number(body: u32, ordinal: usize) -> Result<u32, EditError> {
    let distance = u32::try_from(ordinal).map_err(|_| InspectError::StructureLimit)?;
    body.checked_add(1)
        .and_then(|number| number.checked_add(distance))
        .ok_or_else(|| InspectError::StructureLimit.into())
}

fn inline(
    base: u32,
    members: &[u32],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<u8>, EditError> {
    base.checked_add(INLINE_PAGES - 1)
        .ok_or(InspectError::StructureLimit)?;
    let mut after = zeros(INLINE_RECORD_BYTES, reader)?;
    after[1..INLINE_PREFIX].copy_from_slice(&base.to_le_bytes());
    for &member in members {
        reader.tick()?;
        let relative = member
            .checked_sub(base)
            .filter(|relative| *relative < INLINE_PAGES)
            .ok_or(EditError::Preservation)?;
        after[INLINE_PREFIX + relative as usize / u8::BITS as usize] |= 1 << (relative % u8::BITS);
    }
    Ok(after)
}

fn set_bitmap(
    bytes: &mut [u8],
    number: u32,
    block: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.tick()?;
    if number / BITMAP_PAGES != block {
        return Err(EditError::Preservation);
    }
    let relative = number % BITMAP_PAGES;
    let at = BITMAP_PREFIX + relative as usize / u8::BITS as usize;
    let byte = bytes.get_mut(at).ok_or(EditError::Preservation)?;
    let mask = 1 << (relative % u8::BITS);
    if *byte & mask != 0 {
        return Err(EditError::Preservation);
    }
    *byte |= mask;
    Ok(())
}

fn zeros(length: usize, reader: &mut Reader<'_, Cursor<&[u8]>>) -> Result<Vec<u8>, EditError> {
    let mut bytes = Vec::new();
    reserve(&mut bytes, length, reader.budget)?;
    reader.work(length)?;
    bytes.resize(length, 0);
    Ok(bytes)
}

fn copied(bytes: &[u8], reader: &mut Reader<'_, Cursor<&[u8]>>) -> Result<Vec<u8>, EditError> {
    let mut result = Vec::new();
    reserve(&mut result, bytes.len(), reader.budget)?;
    reader.work(bytes.len())?;
    result.extend_from_slice(bytes);
    Ok(result)
}

fn copied_members(
    members: &[u32],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<u32>, EditError> {
    let mut result = Vec::new();
    reserve(&mut result, members.len(), reader.budget)?;
    reader.work(members.len())?;
    result.extend_from_slice(members);
    Ok(result)
}
