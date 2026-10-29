//! INSERT-owned window growth and existing indirect block-zero initialization.
mod record;
use crate::{
    EditError, InspectError, TableSchema, WriteBlocker,
    format::{
        allocation::{MapRole, QualifiedInlineRecord, RepresentedMap},
        page::{PAGE_BYTES, USAGE_SIGNATURE},
        reader::Reader,
    },
    table::mutation::{
        append::{self, State},
        map_authority::Owner,
        variable_split::metadata::Metadata,
    },
};
use record::Record;
use std::io::Cursor;

const BLOCK: u32 = 0;
const BITMAP_PREFIX: usize = USAGE_SIGNATURE.len();
const BITMAP_PAGES: u32 = ((PAGE_BYTES - BITMAP_PREFIX) * u8::BITS as usize) as u32;

struct Bitmap {
    number: u32,
    bytes: Vec<u8>,
}

pub(super) struct Owned {
    record: Record,
    base: u32,
    rewritten: bool,
    bitmap: Option<Bitmap>,
}

impl Owned {
    pub(super) fn capture(
        schema: &TableSchema,
        metadata: &Metadata,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Option<Self>, EditError> {
        let usage = schema.layout.usage;
        let map = RepresentedMap::read(reader, usage, MapRole::Owned)?;
        let Some(record) = Record::capture(usage, map.kind(), reader)? else {
            return Ok(None);
        };
        metadata.require_record_rewrite(
            Owner::OrdinaryOwned {
                definition: schema.definition_page(),
            },
            usage,
            record.range(),
            reader.budget,
            reader.cancelled,
        )?;
        let base = record.start()?;
        Ok(Some(Self {
            record,
            base,
            rewritten: false,
            bitmap: None,
        }))
    }

    pub(super) fn needs_rewrite(&self, number: u32) -> bool {
        self.record.needs_initialization()
            || self.bitmap.is_some()
            || self.rewritten
            || number
                .checked_sub(self.base)
                .is_none_or(|relative| relative >= QualifiedInlineRecord::PAGE_CAPACITY as u32)
    }

    pub(super) fn add(
        &mut self,
        number: u32,
        members: &[u32],
        append: &mut State,
        global: &RepresentedMap,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        reader.work(members.len())?;
        if members.last().is_some_and(|last| *last >= number) {
            return Err(EditError::Preservation);
        }
        if let Some(bitmap) = &mut self.bitmap {
            return set_member(&mut bitmap.bytes, number, bitmap.number, reader);
        }
        if self.record.needs_initialization() {
            return self.promote(number, members, append, global, reader);
        }
        let minimum = members.first().copied().unwrap_or(number);
        let represented = number
            .checked_sub(self.base)
            .is_some_and(|relative| relative < QualifiedInlineRecord::PAGE_CAPACITY as u32);
        let base = if represented {
            self.base
        } else {
            minimum - minimum % u8::BITS
        };
        let span = number
            .checked_sub(base)
            .and_then(|distance| distance.checked_add(1))
            .ok_or(InspectError::StructureLimit)?;
        if !represented && span > QualifiedInlineRecord::PAGE_CAPACITY as u32 {
            return self.promote(number, members, append, global, reader);
        }
        reader
            .budget
            .charge_allocation((members.len() + 1) as u64, size_of::<u32>() as u64)
            .map_err(InspectError::from)?;
        let mut complete = Vec::new();
        complete
            .try_reserve_exact(members.len() + 1)
            .map_err(|_| InspectError::Allocation)?;
        complete.extend_from_slice(members);
        complete.push(number);
        let after = self.record.inline(base, &complete, reader)?;
        append::rewrite_record(
            &mut append.map_edits,
            self.record.range().start,
            self.record.before(),
            &after,
            reader,
        )?;
        self.base = base;
        self.rewritten = true;
        Ok(())
    }

    pub(super) fn metadata_page(&self) -> Option<(u32, u32, u32, &[u8])> {
        self.bitmap.as_ref().map(|bitmap| {
            (
                bitmap.number,
                self.record.reference(),
                BLOCK,
                bitmap.bytes.as_slice(),
            )
        })
    }

    fn promote(
        &mut self,
        number: u32,
        members: &[u32],
        append: &mut State,
        global: &RepresentedMap,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        let metadata = append.next()?;
        append.preview(1)?;
        let mut bytes = Vec::new();
        reader
            .budget
            .charge_allocation(PAGE_BYTES as u64, 1)
            .map_err(InspectError::from)?;
        bytes
            .try_reserve_exact(PAGE_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        reader.work(PAGE_BYTES)?;
        bytes.resize(PAGE_BYTES, 0);
        bytes[..BITMAP_PREFIX].copy_from_slice(&USAGE_SIGNATURE);
        for member in members.iter().copied().chain([number]) {
            set_member(&mut bytes, member, metadata, reader)?;
        }
        let after = self.record.indirect(metadata, reader)?;
        append.claim_global(global, metadata, reader)?;
        append.reserve_owned_bitmap(
            metadata,
            self.record.reference(),
            BLOCK,
            reader.budget,
            reader.cancelled,
        )?;
        append::rewrite_record(
            &mut append.map_edits,
            self.record.range().start,
            self.record.before(),
            &after,
            reader,
        )?;
        self.bitmap = Some(Bitmap {
            number: metadata,
            bytes,
        });
        self.rewritten = true;
        Ok(())
    }
}

fn set_member(
    bytes: &mut [u8],
    number: u32,
    metadata: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.tick()?;
    if number >= BITMAP_PAGES {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    if number == metadata {
        return Err(EditError::Preservation);
    }
    let at = BITMAP_PREFIX + number as usize / u8::BITS as usize;
    let mask = 1 << (number % u8::BITS);
    let byte = bytes.get_mut(at).ok_or(EditError::Preservation)?;
    if *byte & mask != 0 {
        return Err(EditError::Preservation);
    }
    *byte |= mask;
    Ok(())
}
