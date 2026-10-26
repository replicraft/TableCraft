//! Locate slot/count patches through checked metadata and ordinary row traversal.
use super::{EditError, RowDelete, rows};
use crate::{
    InspectError, NativeReadSource, TableSchema,
    format::{
        bytes::u32_at,
        definition,
        layout::RECORD_COUNT_OFFSET,
        page::PAGE_BYTES,
        reader::Reader,
        row::{DELETED_MASK, RELOCATED_MASK},
    },
};

const PATCH_BYTES: usize = size_of::<u32>();

pub(super) struct Patch {
    offset: usize,
    before: [u8; PATCH_BYTES],
    after: [u8; PATCH_BYTES],
    length: usize,
}

impl super::super::patch::BytePatch for Patch {
    fn offset(&self) -> usize {
        self.offset
    }
    fn before(&self) -> &[u8] {
        &self.before[..self.length]
    }
    fn after(&self) -> &[u8] {
        &self.after[..self.length]
    }
    fn requires_structural_check(&self) -> bool {
        true
    }
}

pub(super) fn records<R: NativeReadSource>(
    reader: &mut Reader<'_, R>,
    schema: &TableSchema,
    deletes: &[RowDelete],
) -> Result<(Vec<Patch>, Vec<u32>), EditError> {
    reader
        .budget
        .charge_allocation((deletes.len() + 1) as u64, size_of::<Patch>() as u64)
        .map_err(InspectError::from)?;
    reader
        .budget
        .charge_allocation(deletes.len() as u64, size_of::<u32>() as u64)
        .map_err(InspectError::from)?;
    let mut patches = Vec::new();
    let mut references = Vec::new();
    patches
        .try_reserve_exact(deletes.len() + 1)
        .map_err(|_| InspectError::Allocation)?;
    references
        .try_reserve_exact(deletes.len())
        .map_err(|_| InspectError::Allocation)?;
    rows::visit(reader, schema, deletes, |row, request| {
        if row.data != request.expected_bytes() {
            return Err(EditError::Conflict);
        }
        let mut before = [0; PATCH_BYTES];
        let mut after = [0; PATCH_BYTES];
        let length = size_of::<u16>();
        before[..length].copy_from_slice(&row.slot_word.to_le_bytes());
        // Independent producer observations establish both native tombstone bits;
        // payload offset, record contents and every neighboring slot stay fixed.
        let tombstone = row.slot_word | DELETED_MASK | RELOCATED_MASK;
        after[..length].copy_from_slice(&tombstone.to_le_bytes());
        patches.push(Patch {
            offset: row.slot_offset,
            before,
            after,
            length,
        });
        references.push(row.reference);
        Ok(())
    })?;
    // Count bytes always belong to the first definition page. Validate the full
    // logical definition chain before resolving that bounded first-page span.
    let definition = definition::read(reader, schema.definition)?;
    let stored = u32_at(&definition, RECORD_COUNT_OFFSET)?;
    if stored != schema.declared_rows() {
        return Err(InspectError::Malformed.into());
    }
    let removed = u32::try_from(deletes.len()).map_err(|_| InspectError::StructureLimit)?;
    let remaining = stored.checked_sub(removed).ok_or(EditError::Conflict)?;
    if removed != 0 {
        let start = (schema.definition as usize)
            .checked_mul(PAGE_BYTES)
            .and_then(|offset| offset.checked_add(RECORD_COUNT_OFFSET))
            .ok_or(InspectError::StructureLimit)?;
        patches.push(Patch {
            offset: start,
            before: stored.to_le_bytes(),
            after: remaining.to_le_bytes(),
            length: size_of::<u32>(),
        });
    }
    reader.tick()?;
    Ok((patches, references))
}
