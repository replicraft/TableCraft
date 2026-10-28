//! Ignored membership derived from the complete primary logical-ROW universe.
use super::{Wire, reference, reserve, selection::Selection};
use crate::{
    EditError, InspectError, TableCheck,
    format::{
        reader::Reader,
        row::{reference_page, reference_slot},
    },
    table::record,
};
use std::io::Cursor;
use tablecraft_core::limits::Resource;

#[cfg(test)]
mod tests;

pub(super) fn primary_references(
    checked: &TableCheck,
    selections: &[Selection<'_>],
    sources: &[(Vec<Wire>, Vec<u32>)],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Option<Vec<u32>>, EditError> {
    let mut ignored = false;
    for selection in selections {
        reader.tick()?;
        ignored |= selection.profile.ignore_nulls;
    }
    if !ignored {
        return Ok(None);
    }
    let mut primary = None;
    for (position, selection) in selections.iter().enumerate() {
        reader.tick()?;
        if selection.primary && primary.replace(position).is_some() {
            return Err(EditError::Preservation);
        }
    }
    let position = primary.ok_or(EditError::Preservation)?;
    let selection = &selections[position];
    let entries = &sources.get(position).ok_or(EditError::Preservation)?.0;
    if selection.profile.ignore_nulls || entries.len() != checked.rows() {
        return Err(EditError::Preservation);
    }
    let mut references = Vec::new();
    reserve(&mut references, entries.len(), reader)?;
    let mut previous = None;
    for entry in entries {
        reader.work(entry.len())?;
        let value = selection
            .profile
            .value(entry)?
            .ok_or(EditError::Preservation)?;
        if previous.is_some_and(|prior| prior >= value) {
            return Err(EditError::Preservation);
        }
        previous = Some(value);
        references.push(reference(entry));
    }
    let levels = references.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let work = references
        .len()
        .checked_mul(levels)
        .ok_or(InspectError::StructureLimit)?;
    reader.work(work)?;
    references.sort_unstable();
    for pair in references.windows(2) {
        reader.tick()?;
        if pair[0] >= pair[1] {
            return Err(EditError::Preservation);
        }
    }
    Ok(Some(references))
}

pub(super) fn original(
    checked: &TableCheck,
    selection: &Selection<'_>,
    references: &[u32],
    entries: &[Wire],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let mut expected = Vec::new();
    reserve(&mut expected, references.len(), reader)?;
    let mut current = None;
    for reference in references {
        reader.tick()?;
        reader
            .budget
            .charge(Resource::Rows, 1)
            .map_err(InspectError::from)?;
        let number = reference_page(*reference);
        if !matches!(&current, Some((page, _)) if *page == number) {
            current = Some((number, reader.page(number)?));
        }
        let (_, page) = current.as_ref().ok_or(EditError::Preservation)?;
        let raw = reader.row_from_page(
            number,
            reference_slot(*reference),
            checked.schema().definition_page(),
            page,
        )?;
        let values =
            record::decode_selected(&raw, checked.schema(), Some(&[selection.column]), reader)?;
        let value = selection
            .profile
            .request(values.first().ok_or(EditError::Preservation)?)?;
        if values.len() != 1 {
            return Err(EditError::Preservation);
        }
        if value.is_some() {
            reader.work(size_of::<Wire>())?;
            expected.push(selection.profile.encode(value, *reference)?);
        }
    }
    let levels = expected.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let work = expected
        .len()
        .checked_mul(levels)
        .and_then(|work| work.checked_mul(size_of::<Wire>()))
        .ok_or(InspectError::StructureLimit)?;
    reader.work(work)?;
    expected.sort_unstable();
    reader.work(
        expected
            .len()
            .checked_mul(size_of::<Wire>())
            .ok_or(InspectError::StructureLimit)?,
    )?;
    if expected != entries {
        return Err(EditError::Preservation);
    }
    Ok(())
}
