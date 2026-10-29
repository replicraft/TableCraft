//! Only original omitted keys can act as new historical UPDATE insertions.
use super::{
    super::{IndexDataPatch, profile::WriteProfile},
    Transition, reserve,
};
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableSchema, WriteBlocker,
    format::reader::Reader, inspect_table_index_counter, table::indexes::page::KeyRef,
};
use std::io::Cursor;

pub(super) fn increment<P: WriteProfile>(
    transitions: &[Transition<P::Key>],
    original: &[KeyRef<P::Key>],
    profile: &P,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<usize, EditError> {
    let mut introduced = Vec::new();
    reserve(&mut introduced, transitions.len(), reader)?;
    for change in transitions {
        reader.tick()?;
        if !profile.constraint_nulls(&change.before)?.0
            || profile.constraint_nulls(&change.after)?.0
        {
            continue;
        }
        let levels = original.len().checked_ilog2().unwrap_or(0) as usize + 1;
        reader.work(levels.saturating_mul(profile.max_key_bytes()))?;
        let existed = original
            .binary_search_by(|entry| profile.compare(&entry.key, &change.after))
            .is_ok();
        let mut original_family_stays = false;
        let mut represented_replacement_introduces = false;
        for other in transitions {
            reader.tick()?;
            reader.work(profile.max_key_bytes().saturating_mul(2))?;
            if !profile.constraint_nulls(&other.before)?.0
                && profile.compare(&other.after, &change.after).is_eq()
            {
                original_family_stays |= profile.compare(&other.before, &change.after).is_eq();
                represented_replacement_introduces = true;
            }
        }
        // A replacement never increments history. An omitted row's insertion
        // can do so. Do not invent timing when both touch the same family.
        if (existed && !original_family_stays) || (!existed && represented_replacement_introduces) {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        if existed {
            continue;
        }
        reader.work(introduced.len().saturating_mul(profile.max_key_bytes()))?;
        if !introduced
            .iter()
            .any(|prior| profile.compare(prior, &change.after).is_eq())
        {
            introduced.push(change.after);
        }
    }
    Ok(introduced.len())
}

pub(in crate::table::indexes::mutation) fn patch(
    image: &NativeImage,
    schema: &TableSchema,
    index: &NativeIndex,
    increment: usize,
    patches: &mut Vec<IndexDataPatch>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    if increment == 0 {
        return Ok(());
    }
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
    {
        return Err(EditError::Preservation);
    }
    let delta = u32::try_from(increment).map_err(|_| InspectError::StructureLimit)?;
    let after = historical
        .value()
        .checked_add(delta)
        .ok_or(InspectError::StructureLimit)?;
    let range = historical.byte_range();
    let before = historical.value().to_le_bytes();
    if image.as_bytes().get(range.clone()) != Some(before.as_slice()) {
        return Err(EditError::Preservation);
    }
    reserve(patches, 1, reader)?;
    let mut before_bytes = Vec::new();
    let mut after_bytes = Vec::new();
    reserve(&mut before_bytes, size_of::<u32>(), reader)?;
    reserve(&mut after_bytes, size_of::<u32>(), reader)?;
    before_bytes.extend_from_slice(&before);
    after_bytes.extend_from_slice(&after.to_le_bytes());
    patches.push(IndexDataPatch {
        offset: range.start,
        before: before_bytes,
        after: after_bytes,
    });
    Ok(())
}
