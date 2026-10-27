//! Current candidate membership never replaces original row-byte authority.
use crate::{
    EditError, InspectError, WriteBlocker,
    format::{
        allocation::{self, MapCell, QualifiedInlineRecord, RepresentedMap},
        page::{PAGE_BYTES, USAGE_SIGNATURE},
        reader::Reader,
    },
    table::mutation::{
        append::{self, State},
        map_authority::Owner,
        variable_split::metadata::Metadata,
    },
};
use std::io::Cursor;

const INLINE_PREFIX_BYTES: usize = 1 + size_of::<u32>();
const BITMAP_PREFIX_BYTES: usize = USAGE_SIGNATURE.len();
const BITMAP_CAPACITY: usize = (PAGE_BYTES - BITMAP_PREFIX_BYTES) * u8::BITS as usize;

enum Layout {
    Original,
    Rebased { start: u32 },
    Promoted { number: u32, bytes: Vec<u8> },
}

pub(super) struct Hint {
    record: QualifiedInlineRecord,
    members: Vec<u32>,
    layout: Layout,
}

impl Hint {
    pub(super) fn capture(
        definition: u32,
        reference: u32,
        metadata: &Metadata,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Self, EditError> {
        let record = QualifiedInlineRecord::capture(reader, reference)?;
        metadata.require_record_rewrite(
            Owner::OrdinaryCandidate { definition },
            record.reference(),
            record.range(),
            reader.budget,
            reader.cancelled,
        )?;
        let members = allocation::pages(reader, reference)?;
        for pair in members.windows(2) {
            reader.tick()?;
            if pair[0] >= pair[1] {
                return Err(InspectError::Malformed.into());
            }
        }

        Ok(Self {
            record,
            members,
            layout: Layout::Original,
        })
    }

    pub(super) fn members(&self) -> &[u32] {
        &self.members
    }

    pub(super) fn highest(
        &self,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Option<u32>, EditError> {
        reader.tick()?;
        Ok(self.members.last().copied())
    }

    pub(super) fn metadata_page(&self) -> Option<(u32, u32, &[u8])> {
        match &self.layout {
            Layout::Promoted { number, bytes } => Some((*number, self.record.reference(), bytes)),
            _ => None,
        }
    }

    pub(super) fn remove(
        &mut self,
        number: u32,
        append: &mut State,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        reader.work(self.members.len())?;
        let position = self
            .members
            .iter()
            .position(|member| *member == number)
            .ok_or(EditError::Preservation)?;

        match &mut self.layout {
            Layout::Original => {
                let cell = original_cell(&self.record, number)?;
                append::clear_hint(&mut append.map_edits, cell, reader)?;
            }
            Layout::Rebased { start } => {
                let mut members = copied(&self.members, 0, reader)?;
                members.remove(position);
                let after = self.record.inline(*start, &members, reader)?;
                append::rewrite_record(
                    &mut append.map_edits,
                    self.record.range().start,
                    self.record.before(),
                    &after,
                    reader,
                )?;
            }
            Layout::Promoted { bytes, .. } => {
                let (offset, mask) = bitmap_cell(number)?;
                reader.tick()?;
                let byte = bytes.get_mut(offset).ok_or(EditError::Preservation)?;
                if *byte & mask == 0 {
                    return Err(EditError::Preservation);
                }
                *byte &= !mask;
            }
        }

        self.members.remove(position);
        Ok(())
    }

    pub(super) fn add(
        &mut self,
        number: u32,
        global: &RepresentedMap,
        append: &mut State,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        reader.tick()?;
        if self.members.last().is_some_and(|last| *last >= number) {
            return Err(EditError::Preservation);
        }
        let mut members = copied(&self.members, 1, reader)?;
        members.push(number);

        let start = match &self.layout {
            Layout::Original => self.record.start()?,
            Layout::Rebased { start } => *start,
            Layout::Promoted { number: bitmap, .. } => {
                if *bitmap == number {
                    return Err(EditError::Preservation);
                }
                let (offset, mask) = bitmap_cell(number)?;
                let Layout::Promoted { bytes, .. } = &mut self.layout else {
                    return Err(EditError::Preservation);
                };
                reader.tick()?;
                let byte = bytes.get_mut(offset).ok_or(EditError::Preservation)?;
                if *byte & mask != 0 {
                    return Err(EditError::Preservation);
                }
                *byte |= mask;
                self.members = members;
                return Ok(());
            }
        };
        // A represented edge bit remains a normal update. The stricter shift
        // threshold applies only after the current window cannot hold it.
        if number
            .checked_sub(start)
            .is_some_and(|relative| relative < QualifiedInlineRecord::PAGE_CAPACITY as u32)
        {
            match self.layout {
                Layout::Original => {
                    let cell = original_cell(&self.record, number)?;
                    append::claim(&mut append.map_edits, cell, true, reader)?;
                }
                Layout::Rebased { .. } => {
                    let after = self.record.inline(start, &members, reader)?;
                    append::rewrite_record(
                        &mut append.map_edits,
                        self.record.range().start,
                        self.record.before(),
                        &after,
                        reader,
                    )?;
                }
                Layout::Promoted { .. } => return Err(EditError::Preservation),
            }
            self.members = members;
            return Ok(());
        }

        let first = *members.first().ok_or(EditError::Preservation)?;
        let aligned = first / u8::BITS * u8::BITS;
        let span = number
            .checked_sub(aligned)
            .and_then(|span| span.checked_add(1))
            .ok_or(InspectError::Malformed)?;
        if span < QualifiedInlineRecord::PAGE_CAPACITY as u32 {
            let after = self.record.inline(aligned, &members, reader)?;
            append::rewrite_record(
                &mut append.map_edits,
                self.record.range().start,
                self.record.before(),
                &after,
                reader,
            )?;
            self.layout = Layout::Rebased { start: aligned };
            self.members = members;
            return Ok(());
        }

        let metadata = append.next()?;
        append.preview(1)?;
        let cell = global.future_cell(reader, metadata)?;
        if !append::current(&append.map_edits, &cell, reader)? {
            return Err(EditError::Preservation);
        }
        reader
            .budget
            .charge_allocation(PAGE_BYTES as u64, 1)
            .map_err(InspectError::from)?;
        reader.work(PAGE_BYTES)?;
        let mut bitmap = Vec::new();
        bitmap
            .try_reserve_exact(PAGE_BYTES)
            .map_err(|_| InspectError::Allocation)?;
        bitmap.resize(PAGE_BYTES, 0);
        bitmap[..USAGE_SIGNATURE.len()].copy_from_slice(&USAGE_SIGNATURE);
        for &member in &members {
            reader.tick()?;
            let (offset, mask) = bitmap_cell(member)?;
            if bitmap[offset] & mask != 0 {
                return Err(EditError::Preservation);
            }
            bitmap[offset] |= mask;
        }
        let after = self.record.indirect(metadata, reader)?;
        append::rewrite_record(
            &mut append.map_edits,
            self.record.range().start,
            self.record.before(),
            &after,
            reader,
        )?;
        append::claim(&mut append.map_edits, cell, false, reader)?;
        append.reserve_candidate_bitmap(
            metadata,
            self.record.reference(),
            reader.budget,
            reader.cancelled,
        )?;
        self.layout = Layout::Promoted {
            number: metadata,
            bytes: bitmap,
        };
        self.members = members;
        Ok(())
    }
}

fn original_cell(record: &QualifiedInlineRecord, number: u32) -> Result<MapCell, EditError> {
    let relative = number
        .checked_sub(record.start()?)
        .filter(|relative| *relative < QualifiedInlineRecord::PAGE_CAPACITY as u32)
        .ok_or(EditError::Preservation)?;
    let within = INLINE_PREFIX_BYTES + relative as usize / u8::BITS as usize;
    Ok(MapCell {
        offset: record.range().start + within,
        before: *record.before().get(within).ok_or(EditError::Preservation)?,
        mask: 1 << (relative % u8::BITS),
    })
}

fn bitmap_cell(number: u32) -> Result<(usize, u8), EditError> {
    if number as usize >= BITMAP_CAPACITY {
        return Err(EditError::Unsupported(WriteBlocker::RowLayout));
    }
    Ok((
        BITMAP_PREFIX_BYTES + number as usize / u8::BITS as usize,
        1 << (number % u8::BITS),
    ))
}

fn copied(
    members: &[u32],
    extra: usize,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<u32>, EditError> {
    let count = members
        .len()
        .checked_add(extra)
        .ok_or(InspectError::StructureLimit)?;
    reader
        .budget
        .charge_allocation(count as u64, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    reader.work(members.len())?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| InspectError::Allocation)?;
    result.extend_from_slice(members);
    Ok(result)
}
