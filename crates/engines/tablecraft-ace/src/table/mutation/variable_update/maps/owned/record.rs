//! Retained original framing for the two qualified ordinary-owned operations.
use crate::{
    InspectError,
    format::{
        allocation::{MapRole, QualifiedInlineRecord, RepresentedMap},
        bytes::u32_at,
        reader::Reader,
        row::{reference_page, reference_slot},
    },
};
use std::{io::Cursor, ops::Range};

const INLINE: u8 = 0;
const INDIRECT: u8 = 1;
const POINTER_START: usize = size_of::<u8>();
const POINTER_END: usize = POINTER_START + size_of::<u32>();
const RECORD_BYTES: usize = POINTER_END + QualifiedInlineRecord::PAGE_CAPACITY / u8::BITS as usize;
const CONTAINER_OWNER: u32 = 0;

pub(super) enum Record {
    Inline(QualifiedInlineRecord),
    Indirect {
        reference: u32,
        offset: usize,
        before: Vec<u8>,
    },
}

impl Record {
    pub(super) fn capture(
        reference: u32,
        kind: u8,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Option<Self>, InspectError> {
        if !matches!(kind, INLINE | INDIRECT) {
            return Ok(None);
        }
        let storage = RepresentedMap::read_storage(reader, reference, MapRole::Owned)?;
        if storage.record.len() != RECORD_BYTES {
            return Ok(None);
        }
        if kind == INLINE {
            return QualifiedInlineRecord::capture(reader, reference)
                .map(Self::Inline)
                .map(Some);
        }

        let number = reference_page(reference);
        let page = reader.page(number)?;
        let row = reader.row_location(number, reference_slot(reference), CONTAINER_OWNER, &page)?;
        if row.data.len() != RECORD_BYTES
            || row.data[0] != INDIRECT
            || u32_at(&row.data, POINTER_START)? != 0
        {
            return Ok(None);
        }
        Ok(Some(Self::Indirect {
            reference,
            offset: row.offset,
            before: row.data,
        }))
    }

    pub(super) fn needs_initialization(&self) -> bool {
        matches!(self, Self::Indirect { .. })
    }

    pub(super) fn reference(&self) -> u32 {
        match self {
            Self::Inline(record) => record.reference(),
            Self::Indirect { reference, .. } => *reference,
        }
    }

    pub(super) fn range(&self) -> Range<usize> {
        match self {
            Self::Inline(record) => record.range(),
            Self::Indirect { offset, before, .. } => *offset..*offset + before.len(),
        }
    }

    pub(super) fn before(&self) -> &[u8] {
        match self {
            Self::Inline(record) => record.before(),
            Self::Indirect { before, .. } => before,
        }
    }

    pub(super) fn start(&self) -> Result<u32, InspectError> {
        match self {
            Self::Inline(record) => record.start(),
            Self::Indirect { .. } => Ok(0),
        }
    }

    pub(super) fn inline(
        &self,
        base: u32,
        members: &[u32],
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Vec<u8>, InspectError> {
        match self {
            Self::Inline(record) => record.inline(base, members, reader),
            Self::Indirect { .. } => Err(InspectError::Unsupported),
        }
    }

    pub(super) fn indirect(
        &self,
        bitmap: u32,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Vec<u8>, InspectError> {
        match self {
            Self::Inline(record) => record.indirect(bitmap, reader),
            Self::Indirect { before, .. } => {
                reader.budget.charge_allocation(before.len() as u64, 1)?;
                let mut after = Vec::new();
                after
                    .try_reserve_exact(before.len())
                    .map_err(|_| InspectError::Allocation)?;
                reader.work(before.len())?;
                after.extend_from_slice(before);
                after[POINTER_START..POINTER_END].copy_from_slice(&bitmap.to_le_bytes());
                Ok(after)
            }
        }
    }
}
