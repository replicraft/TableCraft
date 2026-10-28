//! One table increment and independently derived per-physical historical deltas.
use super::super::super::Staged;
use super::super::ImageProof;
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableCheck,
    format::{bytes::u32_at, layout::RECORD_COUNT_OFFSET, page::PAGE_BYTES, reader::Reader},
    inspect_table_index_counter,
};
use std::io::Cursor;

pub(super) fn prove(
    image: &NativeImage,
    checked: &TableCheck,
    inserted: usize,
    deltas: &[(&NativeIndex, usize)],
    staged: &Staged,
    proof: &mut ImageProof<'_>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let schema = checked.schema();
    let table = schema.definition_page() as usize * PAGE_BYTES + RECORD_COUNT_OFFSET;
    let before = u32_at(image.as_bytes(), table)?;
    if before != schema.declared_rows() || before as usize != checked.rows() {
        return Err(EditError::Preservation);
    }
    let mut position = 0;
    certify(
        table,
        before,
        inserted,
        staged,
        &mut position,
        proof,
        reader,
    )?;
    for (index, delta) in deltas {
        reader.tick()?;
        let historical = inspect_table_index_counter(
            &mut image.reader(),
            image.catalog().catalog().scope(),
            schema.name(),
            index.name(),
            reader.budget,
            reader.cancelled,
        )?;
        if historical.definition_page() != schema.definition_page()
            || historical.physical_index() != index.data_number()
            || historical.byte_range().len() != size_of::<u32>()
        {
            return Err(EditError::Preservation);
        }
        certify(
            historical.byte_range().start,
            historical.value(),
            *delta,
            staged,
            &mut position,
            proof,
            reader,
        )?;
    }
    if position != staged.counters.len() {
        return Err(EditError::Preservation);
    }
    Ok(())
}

fn certify(
    offset: usize,
    value: u32,
    delta: usize,
    staged: &Staged,
    position: &mut usize,
    proof: &mut ImageProof<'_>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.tick()?;
    let before = value.to_le_bytes();
    if proof.original.get(offset..offset + size_of::<u32>()) != Some(before.as_slice()) {
        return Err(EditError::Preservation);
    }
    if delta == 0 {
        return Ok(());
    }
    let delta = u32::try_from(delta).map_err(|_| InspectError::StructureLimit)?;
    let after = value
        .checked_add(delta)
        .ok_or(InspectError::StructureLimit)?
        .to_le_bytes();
    let patch = staged
        .counters
        .get(*position)
        .ok_or(EditError::Preservation)?;
    if patch.offset != offset || patch.before != before || patch.after != after {
        return Err(EditError::Preservation);
    }
    reader.work(size_of::<u32>())?;
    proof
        .bytes
        .get_mut(offset..offset + size_of::<u32>())
        .ok_or(EditError::Preservation)?
        .copy_from_slice(&after);
    *position += 1;
    Ok(())
}
