//! Derive permitted original-association history without producer transition facts.
use super::super::super::super::profile::WriteProfile;
use crate::{
    EditError, InspectError, NativeImage, NativeIndex, TableSchema, WriteBlocker,
    format::reader::Reader, inspect_table_index_counter, table::indexes::page::KeyRef,
};
use std::{io::Cursor, ops::Range};

pub(super) fn delta<P: WriteProfile>(
    original: &[KeyRef<P::Key>],
    transitions: &[(P::Key, P::Key)],
    profile: &P,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<usize, EditError> {
    let mut introduced = Vec::new();
    for &(before, after) in transitions {
        reader.tick()?;
        if !profile.constraint_nulls(&before)?.0 || profile.constraint_nulls(&after)?.0 {
            continue;
        }
        let levels = original.len().checked_ilog2().unwrap_or(0) as usize + 1;
        reader.work(levels.saturating_mul(profile.max_key_bytes()))?;
        let existed = original
            .binary_search_by(|key| profile.compare(&key.key, &after))
            .is_ok();
        let mut retained_family = false;
        let mut replacement = false;
        for &(other_before, other_after) in transitions {
            reader.tick()?;
            reader.work(profile.max_key_bytes().saturating_mul(2))?;
            if !profile.constraint_nulls(&other_before)?.0
                && profile.compare(&other_after, &after).is_eq()
            {
                retained_family |= profile.compare(&other_before, &after).is_eq();
                replacement = true;
            }
        }
        if existed && !retained_family || !existed && replacement {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
        reader.work(introduced.len().saturating_mul(profile.max_key_bytes()))?;
        if !existed
            && !introduced
                .iter()
                .any(|key| profile.compare(key, &after).is_eq())
        {
            super::push(&mut introduced, after, reader)?;
        }
    }
    Ok(introduced.len())
}

pub(super) fn prove(
    image: &NativeImage,
    schema: &TableSchema,
    index: &NativeIndex,
    delta: usize,
    after: &[u8],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Range<usize>, EditError> {
    let counter = inspect_table_index_counter(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        schema.name(),
        index.name(),
        reader.budget,
        reader.cancelled,
    )?;
    if counter.definition_page() != schema.definition_page()
        || counter.physical_index() != index.data_number()
    {
        return Err(EditError::Preservation);
    }
    let delta = u32::try_from(delta).map_err(|_| InspectError::StructureLimit)?;
    let next = counter
        .value()
        .checked_add(delta)
        .ok_or(InspectError::StructureLimit)?;
    let range = counter.byte_range();
    if image.as_bytes().get(range.clone()) != Some(counter.value().to_le_bytes().as_slice())
        || after.get(range.clone()) != Some(next.to_le_bytes().as_slice())
    {
        return Err(EditError::Preservation);
    }
    Ok(range)
}
