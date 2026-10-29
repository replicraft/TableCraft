//! Independent index-owned transitions over the certified DATA ledger and EOF.
use super::super::super::ImageProof;
use super::{PromotionExpected, copied, reserve};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, WriteBlocker,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapCell, MapRole, RepresentedMap},
        bytes::u32_at,
        page::{PAGE_BYTES, USAGE_SIGNATURE},
        reader::Reader,
        row::{reference_page, reference_slot},
    },
    table::mutation::{
        map_authority::{self, Census, Owner, RecordIdentity},
        variable_update::map_audit::{ExpectedByte, expect},
    },
};
use std::io::Cursor;

const INLINE_KIND: u8 = 0;
const INDIRECT_KIND: u8 = 1;
const POINTER_BYTES: usize = size_of::<u32>();
const INLINE_PREFIX: usize = size_of::<u8>() + POINTER_BYTES;
const INLINE_BITMAP_BYTES: usize = 64;
const RECORD_BYTES: usize = INLINE_PREFIX + INLINE_BITMAP_BYTES;
const INLINE_PAGES: u32 = (INLINE_BITMAP_BYTES * u8::BITS as usize) as u32;
const BITMAP_PREFIX: usize = USAGE_SIGNATURE.len();
const BITMAP_PAGES: u32 = ((PAGE_BYTES - BITMAP_PREFIX) * u8::BITS as usize) as u32;

struct Record {
    offset: usize,
    original: Vec<u8>,
}

pub(in crate::table::mutation::insert::audit::primary) struct Allocation<'a> {
    index: &'a NativeIndex,
    owner: u32,
    owned: RepresentedMap,
    census: Census,
    members: Vec<u32>,
    data_offsets: Vec<usize>,
    record: Option<Record>,
    rebased: Option<u32>,
    promoted: Option<PromotionExpected>,
}

impl<'a> Allocation<'a> {
    pub(in crate::table::mutation::insert::audit::primary) fn capture(
        image: &NativeImage,
        index: &'a NativeIndex,
        owner: u32,
        proof: &ImageProof<'_>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Self, EditError> {
        let census = map_authority::capture_catalog_for_initialization(
            image,
            Some((owner, &proof.qualified_zero_columns)),
            reader.budget,
            reader.cancelled,
            |_, _, _, _| Ok(()),
        )?;
        let selected = [
            Owner::GlobalFree,
            Owner::OrdinaryOwned { definition: owner },
            Owner::OrdinaryCandidate { definition: owner },
            Owner::IndexOwned {
                definition: owner,
                physical: index.data_number(),
            },
        ];
        census.require_exclusive(&selected, reader.budget, reader.cancelled)?;
        let global = RepresentedMap::read(reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
        let owned = RepresentedMap::read(reader, index.usage_map(), MapRole::Owned)?;
        global.require_allocated_storage(reader, &global)?;
        owned.require_allocated_storage(reader, &global)?;
        let members = allocation::pages(reader, index.usage_map())?;
        for member in &members {
            reader.tick()?;
            global.require_allocated(reader, *member)?;
        }
        if members.binary_search(&index.root_page()).is_err() {
            return Err(EditError::Preservation);
        }
        let mut data_offsets = Vec::new();
        reserve(&mut data_offsets, proof.map_edits.len(), reader)?;
        for edit in &proof.map_edits {
            reader.tick()?;
            if proof.original.get(edit.offset) != Some(&edit.before)
                || proof.bytes.get(edit.offset) != Some(&edit.after)
            {
                return Err(EditError::Preservation);
            }
            data_offsets.push(edit.offset);
        }

        Ok(Self {
            index,
            owner,
            owned,
            census,
            members,
            data_offsets,
            record: None,
            rebased: None,
            promoted: None,
        })
    }

    pub(in crate::table::mutation::insert::audit::primary) fn reserve(
        &mut self,
        proof: &mut ImageProof<'_>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<u32, EditError> {
        let number = u32::try_from(proof.bytes.len() / PAGE_BYTES)
            .map_err(|_| InspectError::StructureLimit)?;
        reader.work(self.members.len())?;
        if self.members.binary_search(&number).is_ok() {
            return Err(EditError::Preservation);
        }
        if let Some(promotion) = &mut self.promoted {
            set_bitmap(&mut promotion.bitmap, number, reader)?;
            proof.append(number, reader)?;
            reserve(&mut self.members, 1, reader)?;
            self.members.push(number);
            return Ok(number);
        }
        if self.rebased.is_none() {
            match self.owned.future_cell(reader, number) {
                Ok(cell) => {
                    self.protect(cell.offset, cell.offset + 1, reader)?;
                    if cell.before & cell.mask != 0
                        || proof
                            .bytes
                            .get(cell.offset)
                            .is_none_or(|byte| byte & cell.mask != 0)
                    {
                        return Err(EditError::Preservation);
                    }
                    edit(proof, cell, true, reader)?;
                    proof.append(number, reader)?;
                    reserve(&mut self.members, 1, reader)?;
                    self.members.push(number);
                    return Ok(number);
                }
                Err(InspectError::Unsupported) if self.owned.kind() == INLINE_KIND => {}
                Err(error) => return Err(error.into()),
            }
        }

        self.rewrite(number, proof, reader)?;
        Ok(number)
    }

    fn protect(
        &self,
        start: usize,
        end: usize,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        reader.work(self.data_offsets.len())?;
        if self
            .data_offsets
            .iter()
            .any(|offset| start <= *offset && *offset < end)
        {
            return Err(EditError::Preservation);
        }

        Ok(())
    }

    fn rewrite(
        &mut self,
        number: u32,
        proof: &mut ImageProof<'_>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        if self.record.is_none() {
            let page = reference_page(self.index.usage_map());
            let container = reader.page(page)?;
            let row =
                reader.row_location(page, reference_slot(self.index.usage_map()), 0, &container)?;
            if row.data.len() != RECORD_BYTES || row.data[0] != INLINE_KIND {
                return Err(InspectError::Unsupported.into());
            }
            self.record = Some(Record {
                offset: row.offset,
                original: row.data,
            });
        }
        let record = self.record.as_ref().ok_or(EditError::Preservation)?;
        let end = record
            .offset
            .checked_add(RECORD_BYTES)
            .ok_or(EditError::Preservation)?;
        self.protect(record.offset, end, reader)?;
        let index_owner = Owner::IndexOwned {
            definition: self.owner,
            physical: self.index.data_number(),
        };
        self.census.require_record_rewrite(
            &[Owner::GlobalFree, index_owner],
            RecordIdentity {
                owner: index_owner,
                reference: self.index.usage_map(),
                range: record.offset..end,
            },
            reader.budget,
            reader.cancelled,
        )?;
        reader.work(self.members.len())?;
        let minimum = self
            .members
            .iter()
            .copied()
            .chain([number])
            .min()
            .ok_or(EditError::Preservation)?;
        let maximum = self
            .members
            .iter()
            .copied()
            .chain([number])
            .max()
            .ok_or(EditError::Preservation)?;
        let current = self.rebased.unwrap_or(u32_at(&record.original, 1)?);
        let represented = number
            .checked_sub(current)
            .is_some_and(|relative| relative < INLINE_PAGES);
        let aligned = minimum - minimum % u8::BITS;
        let span = u64::from(maximum) - u64::from(aligned) + 1;
        let base = if represented {
            Some(current)
        } else if span < u64::from(INLINE_PAGES) {
            Some(aligned)
        } else {
            None
        };
        if base.is_none() && maximum >= BITMAP_PAGES {
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
        }
        reserve(&mut self.members, 1, reader)?;
        self.members.push(number);

        if let Some(base) = base {
            let mut after = copied(&[0; RECORD_BYTES], reader)?;
            after[1..INLINE_PREFIX].copy_from_slice(&base.to_le_bytes());
            for member in &self.members {
                reader.tick()?;
                let relative = member
                    .checked_sub(base)
                    .filter(|relative| *relative < INLINE_PAGES)
                    .ok_or(EditError::Preservation)?;
                let at = INLINE_PREFIX + relative as usize / u8::BITS as usize;
                after[at] |= 1 << (relative % u8::BITS);
            }
            replace(proof, record, &after, true, reader)?;
            proof.append(number, reader)?;
            self.rebased = Some(base);
            return Ok(());
        }

        proof.append(number, reader)?;
        let metadata = u32::try_from(proof.bytes.len() / PAGE_BYTES)
            .map_err(|_| InspectError::StructureLimit)?;
        let mut bitmap = copied(&[0; PAGE_BYTES], reader)?;
        bitmap[..BITMAP_PREFIX].copy_from_slice(&USAGE_SIGNATURE);
        for member in &self.members {
            set_bitmap(&mut bitmap, *member, reader)?;
        }
        let mut after = copied(&[0; RECORD_BYTES], reader)?;
        after[0] = INDIRECT_KIND;
        after[1..INLINE_PREFIX].copy_from_slice(&metadata.to_le_bytes());
        replace(proof, record, &after, false, reader)?;
        proof.append(metadata, reader)?;
        self.promoted = Some(PromotionExpected {
            number: metadata,
            offset: record.offset,
            before: copied(&record.original, reader)?,
            after,
            bitmap,
        });
        self.rebased = None;

        Ok(())
    }

    pub(in crate::table::mutation::insert::audit::primary) fn finish(
        self,
        proof: &mut ImageProof<'_>,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(Option<PromotionExpected>, Option<u32>), EditError> {
        if let Some(promotion) = &self.promoted {
            let start = promotion.number as usize * PAGE_BYTES;
            reader.work(PAGE_BYTES)?;
            proof.bytes[start..start + PAGE_BYTES].copy_from_slice(&promotion.bitmap);
        }

        Ok((self.promoted, self.rebased))
    }
}

fn edit(
    proof: &mut ImageProof<'_>,
    cell: MapCell,
    set: bool,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    if proof.original.get(cell.offset) != Some(&cell.before) {
        return Err(EditError::Preservation);
    }
    let current = *proof
        .bytes
        .get(cell.offset)
        .ok_or(EditError::Preservation)?;
    let after = if set {
        current | cell.mask
    } else {
        current & !cell.mask
    };
    let offset = cell.offset;
    expect(&mut proof.map_edits, cell, set, reader)?;
    proof.bytes[offset] = after;
    Ok(())
}

fn replace(
    proof: &mut ImageProof<'_>,
    record: &Record,
    after: &[u8],
    inline: bool,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let end = record
        .offset
        .checked_add(RECORD_BYTES)
        .ok_or(EditError::Preservation)?;
    if after.len() != RECORD_BYTES
        || proof.original.get(record.offset..end) != Some(&record.original)
    {
        return Err(EditError::Preservation);
    }
    reader.work(proof.map_edits.len() + RECORD_BYTES)?;
    for edit in &proof.map_edits {
        if record.offset <= edit.offset
            && edit.offset < end
            && record.original[edit.offset - record.offset] != edit.before
        {
            return Err(EditError::Preservation);
        }
    }
    proof
        .map_edits
        .retain(|edit| edit.offset < record.offset || edit.offset >= end);
    if inline {
        reserve(&mut proof.map_edits, RECORD_BYTES, reader)?;
        for (position, (&before, &after)) in record.original.iter().zip(after).enumerate() {
            proof.map_edits.push(ExpectedByte {
                offset: record.offset + position,
                before,
                after,
            });
        }
    }
    proof.bytes[record.offset..end].copy_from_slice(after);

    Ok(())
}

fn set_bitmap(
    bytes: &mut [u8],
    number: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.tick()?;
    if number >= BITMAP_PAGES {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    let at = BITMAP_PREFIX + number as usize / u8::BITS as usize;
    let mask = 1 << (number % u8::BITS);
    if bytes[at] & mask != 0 {
        return Err(EditError::Preservation);
    }
    bytes[at] |= mask;
    Ok(())
}
