//! Reconstruct candidate-map transitions from original bytes and replayed DATA.
use super::{ExpectedByte, expect};
use crate::{
    EditError, InspectError, WriteBlocker,
    format::{
        allocation::{MapCell, MapRole, RepresentedMap},
        bytes::u32_at,
        page::PAGE_BYTES,
        reader::Reader,
        row::{reference_page, reference_slot},
    },
    limits::MAX_IMAGE_BYTES,
    table::mutation::{map_authority::Owner, variable_split::metadata::Metadata},
};
use std::io::Cursor;

const RECORD_BYTES: usize = 69;
const INLINE_PREFIX: usize = 5;
const BITS: u32 = 8;
const INLINE_PAGES: u32 = 512;
const BITMAP_PREFIX: [u8; 4] = [5, 1, 0, 0];
const BITMAP_PAGES: u32 = ((PAGE_BYTES - BITMAP_PREFIX.len()) * 8) as u32;

pub(in crate::table::mutation) struct ExpectedBitmap {
    pub(in crate::table::mutation) number: u32,
    pub(in crate::table::mutation) reference: u32,
    pub(in crate::table::mutation) bytes: Vec<u8>,
}

pub(in crate::table::mutation) struct Hint {
    reference: u32,
    offset: usize,
    original: Vec<u8>,
    base: u32,
    members: Vec<u32>,
    rewritten: bool,
    bitmap: Option<ExpectedBitmap>,
}

impl Hint {
    pub(in crate::table::mutation) fn capture(
        definition: u32,
        reference: u32,
        metadata: &Metadata,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Option<Self>, EditError> {
        // Reuse only the existing read grammar. This grants neither rewrite
        // authority nor the builder's transition/membership decisions.
        let represented = RepresentedMap::read(reader, reference, MapRole::TableFreeSpace)?;
        if represented.kind() != 0 {
            return Ok(None);
        }
        let container = reference_page(reference);
        let page = reader.page(container)?;
        let row = reader.row_location(container, reference_slot(reference), 0, &page)?;
        if row.data.len() != RECORD_BYTES || row.data[0] != 0 {
            return Ok(None);
        }
        let end = row
            .offset
            .checked_add(row.data.len())
            .ok_or(EditError::Preservation)?;
        metadata.require_record_rewrite(
            Owner::OrdinaryCandidate { definition },
            reference,
            row.offset..end,
            reader.budget,
            reader.cancelled,
        )?;
        let base = u32_at(&row.data, 1)?;
        base.checked_add(INLINE_PAGES - 1)
            .ok_or(InspectError::Malformed)?;
        reader.work(RECORD_BYTES - INLINE_PREFIX)?;
        let count = row.data[INLINE_PREFIX..]
            .iter()
            .map(|byte| byte.count_ones() as usize)
            .sum::<usize>();
        reader
            .budget
            .charge_allocation(count as u64, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        let mut members = Vec::new();
        members
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;
        for (position, &byte) in row.data[INLINE_PREFIX..].iter().enumerate() {
            for bit in 0..BITS {
                reader.tick()?;
                if byte & (1 << bit) != 0 {
                    let relative = u32::try_from(position)
                        .ok()
                        .and_then(|position| position.checked_mul(BITS))
                        .and_then(|relative| relative.checked_add(bit))
                        .ok_or(InspectError::StructureLimit)?;
                    members.push(base.checked_add(relative).ok_or(InspectError::Malformed)?);
                }
            }
        }
        Ok(Some(Self {
            reference,
            offset: row.offset,
            original: row.data,
            base,
            members,
            rewritten: false,
            bitmap: None,
        }))
    }

    pub(in crate::table::mutation) fn remove(
        &mut self,
        number: u32,
        expected: &mut Vec<ExpectedByte>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        reader.work(self.members.len())?;
        let position = self
            .members
            .binary_search(&number)
            .map_err(|_| EditError::Preservation)?;
        if let Some(bitmap) = &mut self.bitmap {
            let (at, mask) = bitmap_cell(number)?;
            reader.tick()?;
            let byte = bitmap.bytes.get_mut(at).ok_or(EditError::Preservation)?;
            if *byte & mask == 0 {
                return Err(EditError::Preservation);
            }
            *byte &= !mask;
            self.members.remove(position);
            return Ok(());
        }
        if self.rewritten {
            let mut members = self.updated_members(None, reader)?;
            members.remove(position);
            let after = inline_row(self.base, &members, reader)?;
            replace_expected(self.offset, &self.original, &after, expected, reader)?;
            self.members = members;
        } else {
            let cell = self.original_cell(number)?;
            expect(expected, cell, false, reader)?;
            self.members.remove(position);
        }
        Ok(())
    }

    pub(in crate::table::mutation) fn add(
        &mut self,
        number: u32,
        next_eof: &mut u32,
        expected: &mut Vec<ExpectedByte>,
        global: &RepresentedMap,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Option<u32>, EditError> {
        reader.tick()?;
        let members = self.updated_members(Some(number), reader)?;
        if let Some(bitmap) = &mut self.bitmap {
            if bitmap.number == number {
                return Err(EditError::Preservation);
            }
            let (at, mask) = bitmap_cell(number)?;
            reader.tick()?;
            let byte = bitmap.bytes.get_mut(at).ok_or(EditError::Preservation)?;
            if *byte & mask != 0 {
                return Err(EditError::Preservation);
            }
            *byte |= mask;
            self.members = members;
            return Ok(None);
        }
        let within = number
            .checked_sub(self.base)
            .is_some_and(|relative| relative < INLINE_PAGES);
        if within {
            if self.rewritten {
                let after = inline_row(self.base, &members, reader)?;
                replace_expected(self.offset, &self.original, &after, expected, reader)?;
            } else {
                expect(expected, self.original_cell(number)?, true, reader)?;
            }
            self.members = members;
            return Ok(None);
        }
        let first = *members.first().ok_or(EditError::Preservation)?;
        let last = *members.last().ok_or(EditError::Preservation)?;
        let aligned = first - first % BITS;
        let span = last
            .checked_sub(aligned)
            .and_then(|difference| difference.checked_add(1))
            .ok_or(EditError::Preservation)?;
        if span < INLINE_PAGES {
            let after = inline_row(aligned, &members, reader)?;
            replace_expected(self.offset, &self.original, &after, expected, reader)?;
            self.base = aligned;
            self.rewritten = true;
            self.members = members;
            return Ok(None);
        }
        let metadata = *next_eof;
        let end = metadata.checked_add(1).ok_or(EditError::Preservation)?;
        if metadata < reader.header.page_count()
            || u64::from(end) * PAGE_BYTES as u64 > MAX_IMAGE_BYTES
            || members.binary_search(&metadata).is_ok()
        {
            return Err(EditError::Preservation);
        }
        let global_cell = global.future_cell(reader, metadata)?;
        reader.work(expected.len())?;
        let current = if let Some(byte) = expected
            .iter()
            .find(|byte| byte.offset == global_cell.offset)
        {
            if byte.before != global_cell.before {
                return Err(EditError::Preservation);
            }
            byte.after
        } else {
            global_cell.before
        };
        if global_cell.before & global_cell.mask == 0 || current & global_cell.mask == 0 {
            return Err(EditError::Preservation);
        }
        let mut bitmap = zeroed(PAGE_BYTES, reader)?;
        bitmap[..BITMAP_PREFIX.len()].copy_from_slice(&BITMAP_PREFIX);
        for &member in &members {
            reader.tick()?;
            let (at, mask) = bitmap_cell(member)?;
            if bitmap[at] & mask != 0 {
                return Err(EditError::Preservation);
            }
            bitmap[at] |= mask;
        }
        let mut after = zeroed(RECORD_BYTES, reader)?;
        after[0] = 1;
        after[1..INLINE_PREFIX].copy_from_slice(&metadata.to_le_bytes());
        replace_expected(self.offset, &self.original, &after, expected, reader)?;
        expect(expected, global_cell, false, reader)?;
        self.members = members;
        self.rewritten = true;
        self.bitmap = Some(ExpectedBitmap {
            number: metadata,
            reference: self.reference,
            bytes: bitmap,
        });
        *next_eof = end;
        Ok(Some(metadata))
    }

    pub(in crate::table::mutation) fn into_metadata(self) -> Option<ExpectedBitmap> {
        self.bitmap
    }

    fn original_cell(&self, number: u32) -> Result<MapCell, EditError> {
        let original_base = u32_at(&self.original, 1)?;
        let relative = number
            .checked_sub(original_base)
            .filter(|relative| *relative < INLINE_PAGES)
            .ok_or(EditError::Preservation)?;
        let position = INLINE_PREFIX + relative as usize / BITS as usize;
        Ok(MapCell {
            offset: self.offset + position,
            before: self.original[position],
            mask: 1 << (relative % BITS),
        })
    }

    fn updated_members(
        &self,
        addition: Option<u32>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Vec<u32>, EditError> {
        reader.work(self.members.len())?;
        let insertion = if let Some(number) = addition {
            Some(
                self.members
                    .binary_search(&number)
                    .err()
                    .ok_or(EditError::Preservation)?,
            )
        } else {
            None
        };
        let count = self
            .members
            .len()
            .checked_add(usize::from(addition.is_some()))
            .ok_or(InspectError::StructureLimit)?;
        reader
            .budget
            .charge_allocation(count as u64, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;
        result.extend_from_slice(&self.members);
        if let (Some(position), Some(number)) = (insertion, addition) {
            result.insert(position, number);
        }
        Ok(result)
    }
}

fn inline_row(
    base: u32,
    members: &[u32],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<u8>, EditError> {
    if !base.is_multiple_of(BITS) {
        return Err(EditError::Preservation);
    }
    let mut row = zeroed(RECORD_BYTES, reader)?;
    row[1..INLINE_PREFIX].copy_from_slice(&base.to_le_bytes());
    for &member in members {
        reader.tick()?;
        let relative = member
            .checked_sub(base)
            .filter(|relative| *relative < INLINE_PAGES)
            .ok_or(EditError::Preservation)?;
        let position = INLINE_PREFIX + relative as usize / BITS as usize;
        let mask = 1 << (relative % BITS);
        if row[position] & mask != 0 {
            return Err(EditError::Preservation);
        }
        row[position] |= mask;
    }
    Ok(row)
}

fn bitmap_cell(number: u32) -> Result<(usize, u8), EditError> {
    if number >= BITMAP_PAGES {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    Ok((
        BITMAP_PREFIX.len() + number as usize / BITS as usize,
        1 << (number % BITS),
    ))
}

fn zeroed(length: usize, reader: &mut Reader<'_, Cursor<&[u8]>>) -> Result<Vec<u8>, EditError> {
    reader.tick()?;
    reader
        .budget
        .charge_allocation(length as u64, 1)
        .map_err(InspectError::from)?;
    reader.work(length)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    result.resize(length, 0);
    Ok(result)
}

fn replace_expected(
    offset: usize,
    before: &[u8],
    after: &[u8],
    expected: &mut Vec<ExpectedByte>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    if before.len() != RECORD_BYTES || after.len() != RECORD_BYTES {
        return Err(EditError::Preservation);
    }
    let end = offset
        .checked_add(RECORD_BYTES)
        .ok_or(EditError::Preservation)?;
    reader.work(expected.len())?;
    for byte in expected.iter() {
        reader.tick()?;
        if (offset..end).contains(&byte.offset) && byte.before != before[byte.offset - offset] {
            return Err(EditError::Preservation);
        }
    }
    reader
        .budget
        .charge_allocation(RECORD_BYTES as u64, size_of::<ExpectedByte>() as u64)
        .map_err(InspectError::from)?;
    expected
        .try_reserve(RECORD_BYTES)
        .map_err(|_| InspectError::Allocation)?;
    reader.work(RECORD_BYTES * 2)?;
    for _ in 0..RECORD_BYTES {
        reader.tick()?;
    }
    // The freshly captured canonical row proof owns exactly this range. Every
    // byte, even an unchanged byte, seeds later index-overlap protection.
    expected.retain(|byte| !(offset..end).contains(&byte.offset));
    for (position, (&before, &after)) in before.iter().zip(after).enumerate() {
        expected.push(ExpectedByte {
            offset: offset + position,
            before,
            after,
        });
    }
    Ok(())
}
