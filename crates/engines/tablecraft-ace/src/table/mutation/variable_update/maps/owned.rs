//! INSERT-only ordinary-owned inline rebasing preserves the complete member set.
use crate::{
    EditError, InspectError, TableSchema, WriteBlocker,
    format::{
        allocation::{MapRole, QualifiedInlineRecord, RepresentedMap},
        reader::Reader,
    },
    table::mutation::{
        append::{self, State},
        map_authority::Owner,
        variable_split::metadata::Metadata,
    },
};
use std::io::Cursor;

const INLINE: u8 = 0;
const INLINE_RECORD_BYTES: usize =
    1 + size_of::<u32>() + QualifiedInlineRecord::PAGE_CAPACITY / u8::BITS as usize;

pub(super) struct Owned {
    record: QualifiedInlineRecord,
    base: u32,
    rewritten: bool,
}

impl Owned {
    pub(super) fn capture(
        schema: &TableSchema,
        metadata: &Metadata,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<Option<Self>, EditError> {
        let usage = schema.layout.usage;
        let map = RepresentedMap::read(reader, usage, MapRole::Owned)?;
        if map.kind() != INLINE {
            return Ok(None);
        }
        let storage = RepresentedMap::read_storage(reader, usage, MapRole::Owned)?;
        if storage.record.len() != INLINE_RECORD_BYTES {
            return Ok(None);
        }
        let record = QualifiedInlineRecord::capture(reader, usage)?;
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
        }))
    }

    pub(super) fn needs_rewrite(&self, number: u32) -> bool {
        self.rewritten
            || number
                .checked_sub(self.base)
                .is_none_or(|relative| relative >= QualifiedInlineRecord::PAGE_CAPACITY as u32)
    }

    pub(super) fn add(
        &mut self,
        number: u32,
        members: &[u32],
        append: &mut State,
        reader: &mut Reader<'_, Cursor<&[u8]>>,
    ) -> Result<(), EditError> {
        reader.work(members.len())?;
        if members.last().is_some_and(|last| *last >= number) {
            return Err(EditError::Preservation);
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
            return Err(EditError::Unsupported(WriteBlocker::RowLayout));
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
}
