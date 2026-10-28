//! Explicit positive table and historical primary counter deltas.
use super::super::patch::BytePatch;
use crate::{
    EditError, InspectError, NativeImage, TableCheck,
    format::{bytes::u32_at, layout::RECORD_COUNT_OFFSET, page::PAGE_BYTES},
    header::check,
    inspect_table_index_counter,
    table::indexes::mutation::insertion::PrimaryInsertion,
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
    index: &PrimaryInsertion,
    inserted: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Patch>, EditError> {
    check(budget, cancelled)?;
    let delta = u32::try_from(inserted).map_err(|_| InspectError::StructureLimit)?;
    let selected = checked
        .indexes()
        .iter()
        .find(|entry| entry.index().data_number() == index.physical())
        .ok_or(EditError::Preservation)?
        .index();
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
    let table_offset =
        checked.schema().definition_page() as usize * PAGE_BYTES + RECORD_COUNT_OFFSET;
    let value = u32_at(image.as_bytes(), table_offset)?;
    if value != checked.schema().declared_rows() || value as usize != checked.rows() {
        return Err(EditError::Preservation);
    }
    if inserted == 0 {
        return Ok(Vec::new());
    }
    budget
        .charge_allocation(2, size_of::<Patch>() as u64)
        .map_err(InspectError::from)?;
    let mut patches = Vec::new();
    patches
        .try_reserve_exact(2)
        .map_err(|_| InspectError::Allocation)?;
    for (offset, before) in [
        (table_offset, value),
        (historical.byte_range().start, historical.value()),
    ] {
        check(budget, cancelled)?;
        let after = before
            .checked_add(delta)
            .ok_or(InspectError::StructureLimit)?;
        if image.as_bytes().get(offset..offset + size_of::<u32>())
            != Some(before.to_le_bytes().as_slice())
        {
            return Err(EditError::Preservation);
        }
        patches.push(Patch {
            offset,
            before: before.to_le_bytes(),
            after: after.to_le_bytes(),
        });
    }
    Ok(patches)
}
