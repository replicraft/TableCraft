//! One table delta and independently bound per-physical distinct-key deltas.
use super::super::patch::BytePatch;
use crate::{
    EditError, InspectError, NativeImage, TableCheck,
    format::{bytes::u32_at, layout::RECORD_COUNT_OFFSET, page::PAGE_BYTES},
    header::check,
    inspect_table_index_counter,
    table::indexes::mutation::insertion::IndexInsertion,
};
use tablecraft_core::limits::ResourceBudget;

pub(super) struct Patch {
    pub offset: usize,
    pub before: [u8; size_of::<u32>()],
    pub after: [u8; size_of::<u32>()],
}

impl BytePatch for Patch {
    fn offset(&self) -> usize {
        self.offset
    }
    fn before(&self) -> &[u8] {
        &self.before
    }
    fn after(&self) -> &[u8] {
        &self.after
    }
}

pub(super) fn capture(
    image: &NativeImage,
    checked: &TableCheck,
    indexes: &[IndexInsertion],
    inserted: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Patch>, EditError> {
    check(budget, cancelled)?;
    let delta = u32::try_from(inserted).map_err(|_| InspectError::StructureLimit)?;
    if indexes.len() != checked.schema().layout.physical_indexes {
        return Err(EditError::Preservation);
    }
    let table_offset =
        checked.schema().definition_page() as usize * PAGE_BYTES + RECORD_COUNT_OFFSET;
    let value = u32_at(image.as_bytes(), table_offset)?;
    if value != checked.schema().declared_rows() || value as usize != checked.rows() {
        return Err(EditError::Preservation);
    }
    let capacity = indexes
        .len()
        .checked_add(1)
        .ok_or(InspectError::StructureLimit)?;
    budget
        .charge_allocation(capacity as u64, size_of::<Patch>() as u64)
        .map_err(InspectError::from)?;
    let mut patches = Vec::new();
    patches
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    append(image, &mut patches, table_offset, value, delta)?;

    for (position, index) in indexes.iter().enumerate() {
        check(budget, cancelled)?;
        if usize::try_from(index.physical()).map_err(|_| InspectError::StructureLimit)? != position
            || index.distinct_added() > inserted
        {
            return Err(EditError::Preservation);
        }
        let mut selected = None;
        for entry in checked.indexes() {
            check(budget, cancelled)?;
            if entry.index().data_number() == index.physical() {
                selected = Some(entry.index());
                break;
            }
        }
        let selected = selected.ok_or(EditError::Preservation)?;
        let historical = inspect_table_index_counter(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            checked.schema().name(),
            selected.name(),
            budget,
            cancelled,
        )?;
        if historical.definition_page() != checked.schema().definition_page()
            || historical.physical_index() != index.physical()
        {
            return Err(EditError::Preservation);
        }
        let increment =
            u32::try_from(index.distinct_added()).map_err(|_| InspectError::StructureLimit)?;
        append(
            image,
            &mut patches,
            historical.byte_range().start,
            historical.value(),
            increment,
        )?;
    }
    Ok(patches)
}

fn append(
    image: &NativeImage,
    patches: &mut Vec<Patch>,
    offset: usize,
    before: u32,
    increment: u32,
) -> Result<(), EditError> {
    if image.as_bytes().get(offset..offset + size_of::<u32>())
        != Some(before.to_le_bytes().as_slice())
    {
        return Err(EditError::Preservation);
    }
    if increment == 0 {
        return Ok(());
    }
    let after = before
        .checked_add(increment)
        .ok_or(InspectError::StructureLimit)?;
    patches.push(Patch {
        offset,
        before: before.to_le_bytes(),
        after: after.to_le_bytes(),
    });
    Ok(())
}
