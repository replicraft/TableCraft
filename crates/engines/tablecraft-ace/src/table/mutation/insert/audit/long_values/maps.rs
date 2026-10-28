//! Independent column-map rewrites and fresh bitmap placement from source facts.
use super::super::{ImageProof, copied, reserve};
use super::cursor::{change, require_bit};
use crate::{
    EditError, InspectError, LongValueMapReferences, TableSchema,
    format::{
        bytes::u32_at,
        page::PAGE_BYTES,
        reader::Reader,
        row::{reference_page, reference_slot},
    },
    table::mutation::{
        long_value::{
            maps::Scope,
            ownership::{ColumnMapKind, Inventory},
        },
        variable_update::map_audit::ExpectedByte,
    },
};
use std::{io::Cursor, ops::Range};

const INLINE: u8 = 0;
const INDIRECT: u8 = 1;
const POINTER_BYTES: usize = size_of::<u32>();
const INLINE_PREFIX: usize = 1 + POINTER_BYTES;
const INLINE_BYTES: usize = 69;
const INLINE_PAGES: u32 = ((INLINE_BYTES - INLINE_PREFIX) * u8::BITS as usize) as u32;
const BITMAP_HEADER: [u8; 4] = [5, 1, 0, 0];
const BITMAP_PAGES: u32 = ((PAGE_BYTES - BITMAP_HEADER.len()) * u8::BITS as usize) as u32;

struct Record {
    reference: u32,
    range: Range<usize>,
    before: Vec<u8>,
    rewritten: bool,
}

impl Record {
    fn capture(reference: u32, reader: &mut Reader<'_, Cursor<&[u8]>>) -> Result<Self, EditError> {
        let page = reference_page(reference);
        let raw = reader.page(page)?;
        let row = reader.row_location(page, reference_slot(reference), 0, &raw)?;
        let end = row
            .offset
            .checked_add(row.data.len())
            .ok_or(EditError::Preservation)?;
        Ok(Self {
            reference,
            range: row.offset..end,
            before: row.data,
            rewritten: false,
        })
    }

    fn current<'a>(&self, proof: &'a ImageProof<'_>) -> Result<&'a [u8], EditError> {
        proof
            .bytes
            .get(self.range.clone())
            .ok_or(EditError::Preservation)
    }

    fn rewrite(
        &mut self,
        bytes: &[u8],
        authority: Authority<'_>,
        kind: ColumnMapKind,
        proof: &mut ImageProof<'_>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        if bytes.len() != self.before.len() {
            return Err(EditError::Preservation);
        }
        authority.inventory.require_record_rewrite(
            authority.schema,
            authority.column,
            kind,
            (self.reference, self.range.clone()),
            reader.budget,
            reader.cancelled,
        )?;
        // A complete row rewrite absorbs earlier cell changes. Keep its exact
        // original before-image and every byte entry, including unchanged bytes.
        let work = proof
            .map_edits
            .len()
            .checked_mul(2)
            .and_then(|count| count.checked_add(bytes.len()))
            .ok_or(InspectError::StructureLimit)?;
        reader.work(work)?;
        for edit in &proof.map_edits {
            reader.tick()?;
            if self.range.contains(&edit.offset)
                && self.before[edit.offset - self.range.start] != edit.before
            {
                return Err(EditError::Preservation);
            }
        }
        reserve(&mut proof.map_edits, bytes.len(), reader)?;
        proof
            .map_edits
            .retain(|edit| !self.range.contains(&edit.offset));
        for (position, (&before, &after)) in self.before.iter().zip(bytes).enumerate() {
            reader.tick()?;
            proof.map_edits.push(ExpectedByte {
                offset: self.range.start + position,
                before,
                after,
            });
        }
        proof.bytes[self.range.clone()].copy_from_slice(bytes);
        self.rewritten = true;
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub(super) struct Authority<'a> {
    pub schema: &'a TableSchema,
    pub inventory: &'a Inventory,
    pub column: usize,
}

pub(super) struct Allocation<'a> {
    pub authority: Authority<'a>,
    pub scope: &'a Scope,
    pub global: &'a crate::format::allocation::RepresentedMap,
    pub members: &'a [u32],
}

pub(super) struct Bitmap {
    pub number: u32,
    pub column: usize,
    pub owned: u32,
    pub block: u32,
}

pub(super) struct Maps {
    owned: Record,
    candidate: Record,
}

impl Maps {
    pub(super) fn capture(
        references: LongValueMapReferences,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Self, EditError> {
        Ok(Self {
            owned: Record::capture(references.owned(), reader)?,
            candidate: Record::capture(references.candidate(), reader)?,
        })
    }

    pub(super) fn clear_hint(
        &mut self,
        number: u32,
        remaining: &[u32],
        allocation: Allocation<'_>,
        proof: &mut ImageProof<'_>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        if self.candidate.rewritten {
            let base = u32_at(self.candidate.current(proof)?, 1)?;
            let after = inline(base, remaining, reader)?;
            self.candidate.rewrite(
                &after,
                allocation.authority,
                ColumnMapKind::Candidate,
                proof,
                reader,
            )
        } else {
            change(
                proof,
                &allocation.scope.hint,
                reader.header.page_count(),
                number,
                false,
                reader,
            )
        }
    }

    pub(super) fn allocate(
        &mut self,
        allocation: Allocation<'_>,
        bitmaps: &mut Vec<Bitmap>,
        proof: &mut ImageProof<'_>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<u32, EditError> {
        let number = u32::try_from(proof.bytes.len() / PAGE_BYTES)
            .map_err(|_| InspectError::StructureLimit)?;
        let mut members = Vec::new();
        reserve(&mut members, allocation.members.len() + 1, reader)?;
        reader.work(allocation.members.len())?;
        members.extend_from_slice(allocation.members);
        if members.last().is_some_and(|last| *last >= number) {
            return Err(EditError::Preservation);
        }
        members.push(number);
        let mut owned = copied(self.owned.current(proof)?, reader)?;
        let mut blocks = Vec::new();
        let mut rewrite = false;
        let mut represented = false;
        match owned.first().copied() {
            Some(INLINE) => {
                if !self.owned.rewritten {
                    match allocation.scope.owned.future_cell(reader, number) {
                        Ok(_) => represented = true,
                        Err(InspectError::Unsupported) => {}
                        Err(error) => return Err(error.into()),
                    }
                }
                if !represented {
                    if owned.len() != INLINE_BYTES {
                        return Err(EditError::Preservation);
                    }
                    let base = u32_at(&owned, 1)?;
                    let first = *members.first().ok_or(EditError::Preservation)?;
                    let aligned = first - first % u8::BITS;
                    let fits_current = number
                        .checked_sub(base)
                        .is_some_and(|distance| distance < INLINE_PAGES);
                    if fits_current || number - aligned < INLINE_PAGES {
                        owned =
                            inline(if fits_current { base } else { aligned }, &members, reader)?;
                    } else {
                        for member in &members {
                            reader.tick()?;
                            let block = *member / BITMAP_PAGES;
                            if 1 + (block as usize + 1) * POINTER_BYTES > owned.len() {
                                return Err(EditError::Preservation);
                            }
                            if blocks.last() != Some(&block) {
                                reserve(&mut blocks, 1, reader)?;
                                blocks.push(block);
                            }
                        }
                        owned.fill(0);
                        owned[0] = INDIRECT;
                    }
                    rewrite = true;
                }
            }
            Some(INDIRECT) => {
                reader.work(bitmaps.len())?;
                let block = number / BITMAP_PAGES;
                let offset = 1 + block as usize * POINTER_BYTES;
                let pointer = owned
                    .get(offset..offset + POINTER_BYTES)
                    .ok_or(EditError::Preservation)?;
                let pointer =
                    u32::from_le_bytes(pointer.try_into().map_err(|_| EditError::Preservation)?);
                if pointer == 0 {
                    reserve(&mut blocks, 1, reader)?;
                    blocks.push(block);
                    rewrite = true;
                } else if let Some(bitmap) = bitmaps.iter().find(|bitmap| bitmap.number == pointer)
                {
                    if bitmap.column != allocation.authority.column
                        || bitmap.owned != self.owned.reference
                        || bitmap.block != block
                    {
                        return Err(EditError::Preservation);
                    }
                    set_bitmap(proof, pointer, number, reader)?;
                } else {
                    represented = true;
                }
            }
            _ => return Err(EditError::Preservation),
        }

        for distance in 0..=blocks.len() {
            let page = number
                .checked_add(u32::try_from(distance).map_err(|_| InspectError::StructureLimit)?)
                .ok_or(InspectError::StructureLimit)?;
            require_bit(
                proof,
                allocation.global,
                reader.header.page_count(),
                page,
                true,
                reader,
            )?;
        }
        change(
            proof,
            allocation.global,
            reader.header.page_count(),
            number,
            false,
            reader,
        )?;
        proof.append(number, reader)?;
        for (distance, block) in blocks.iter().enumerate() {
            let page = number
                .checked_add(u32::try_from(distance + 1).map_err(|_| InspectError::StructureLimit)?)
                .ok_or(InspectError::StructureLimit)?;
            change(
                proof,
                allocation.global,
                reader.header.page_count(),
                page,
                false,
                reader,
            )?;
            proof.append(page, reader)?;
            let base = page as usize * PAGE_BYTES;
            proof.bytes[base..base + BITMAP_HEADER.len()].copy_from_slice(&BITMAP_HEADER);
            for member in &members {
                reader.tick()?;
                if *member / BITMAP_PAGES == *block {
                    set_bitmap(proof, page, *member, reader)?;
                }
            }
            let at = 1 + *block as usize * POINTER_BYTES;
            owned[at..at + POINTER_BYTES].copy_from_slice(&page.to_le_bytes());
            reserve(bitmaps, 1, reader)?;
            bitmaps.push(Bitmap {
                number: page,
                column: allocation.authority.column,
                owned: self.owned.reference,
                block: *block,
            });
        }
        if represented {
            require_bit(
                proof,
                &allocation.scope.owned,
                reader.header.page_count(),
                number,
                false,
                reader,
            )?;
            change(
                proof,
                &allocation.scope.owned,
                reader.header.page_count(),
                number,
                true,
                reader,
            )?;
        }
        if rewrite {
            self.owned.rewrite(
                &owned,
                allocation.authority,
                ColumnMapKind::Owned,
                proof,
                reader,
            )?;
        }
        let candidate = self.candidate.current(proof)?;
        let candidate_represented = if self.candidate.rewritten {
            false
        } else {
            match allocation.scope.hint.future_cell(reader, number) {
                Ok(_) => true,
                Err(InspectError::Unsupported) => false,
                Err(error) => return Err(error.into()),
            }
        };
        if candidate_represented {
            require_bit(
                proof,
                &allocation.scope.hint,
                reader.header.page_count(),
                number,
                false,
                reader,
            )?;
            change(
                proof,
                &allocation.scope.hint,
                reader.header.page_count(),
                number,
                true,
                reader,
            )?;
        } else {
            if candidate.len() != INLINE_BYTES || candidate[0] != INLINE {
                return Err(EditError::Preservation);
            }
            let old = u32_at(candidate, 1)?;
            let base = if number
                .checked_sub(old)
                .is_some_and(|distance| distance < INLINE_PAGES)
            {
                old
            } else {
                number - number % u8::BITS
            };
            let after = inline(base, &[number], reader)?;
            self.candidate.rewrite(
                &after,
                allocation.authority,
                ColumnMapKind::Candidate,
                proof,
                reader,
            )?;
        }
        Ok(number)
    }
}

fn inline(
    base: u32,
    members: &[u32],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<u8>, EditError> {
    let mut bytes = Vec::new();
    reserve(&mut bytes, INLINE_BYTES, reader)?;
    reader.work(INLINE_BYTES + members.len())?;
    bytes.resize(INLINE_BYTES, 0);
    bytes[1..INLINE_PREFIX].copy_from_slice(&base.to_le_bytes());
    for member in members {
        reader.tick()?;
        let relative = member
            .checked_sub(base)
            .filter(|relative| *relative < INLINE_PAGES)
            .ok_or(EditError::Preservation)?;
        bytes[INLINE_PREFIX + relative as usize / u8::BITS as usize] |= 1 << (relative % u8::BITS);
    }
    Ok(bytes)
}

fn set_bitmap(
    proof: &mut ImageProof<'_>,
    page: u32,
    member: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.tick()?;
    let relative = member % BITMAP_PAGES;
    let offset =
        page as usize * PAGE_BYTES + BITMAP_HEADER.len() + relative as usize / u8::BITS as usize;
    let byte = proof.bytes.get_mut(offset).ok_or(EditError::Preservation)?;
    let mask = 1 << (relative % u8::BITS);
    if *byte & mask != 0 {
        return Err(EditError::Preservation);
    }
    *byte |= mask;
    Ok(())
}
