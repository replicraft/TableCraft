//! Ignored membership derived from the complete primary logical-ROW universe.
use super::{Profile, Wire, profile::Nulls, reference, reserve, selection::Selection};
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
        ignored |= selection.profile.ignore_nulls || selection.profile.variable();
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
        if selection.profile.value(entry)?.any {
            return Err(EditError::Preservation);
        }
        let key = &entry[..entry.len() - size_of::<u32>()];
        if previous.is_some_and(|prior: &[u8]| prior >= key) {
            return Err(EditError::Preservation);
        }
        previous = Some(key);
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

#[derive(Debug, Eq, PartialEq)]
pub(super) struct RowFacts {
    pub reference: u32,
    pub nulls: Nulls,
}

pub(super) fn nulls(
    profile: &Profile<'_>,
    entry: &Wire,
    facts: &[RowFacts],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Nulls, EditError> {
    if !profile.variable() {
        return profile.value(entry);
    }
    let levels = facts.len().checked_ilog2().unwrap_or(0) as usize + 1;
    reader.work(levels)?;
    let position = facts
        .binary_search_by_key(&reference(entry), |fact| fact.reference)
        .map_err(|_| EditError::Preservation)?;
    Ok(facts[position].nulls)
}

pub(super) fn sort_facts(
    facts: &mut [RowFacts],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let levels = facts.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let work = facts
        .len()
        .checked_mul(levels)
        .and_then(|work| work.checked_mul(size_of::<RowFacts>()))
        .ok_or(InspectError::StructureLimit)?;
    reader.work(work)?;
    facts.sort_unstable_by_key(|fact| fact.reference);
    for pair in facts.windows(2) {
        reader.tick()?;
        if pair[0].reference >= pair[1].reference {
            return Err(EditError::Preservation);
        }
    }
    Ok(())
}

pub(super) fn original(
    checked: &TableCheck,
    selection: &Selection<'_>,
    references: &[u32],
    entries: &[Wire],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<RowFacts>, EditError> {
    let mut expected = Vec::new();
    let mut facts = Vec::new();
    if selection.profile.variable() {
        reserve(&mut facts, references.len(), reader)?;
    }
    reserve(&mut expected, references.len(), reader)?;
    // The record decoder emits declaration order, independently of written
    // index component order. Decode/remap a sorted set; the profile retains its
    // original component sequence for physical key assembly.
    let mut columns = selection.profile.columns(reader)?;
    let levels = columns.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let work = columns
        .len()
        .checked_mul(levels)
        .ok_or(InspectError::StructureLimit)?;
    reader.work(work)?;
    columns.sort_unstable();
    reader.work(columns.len())?;
    columns.dedup();
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
        let selected = record::decode_selected(&raw, checked.schema(), Some(&columns), reader)?;
        if selected.len() != columns.len() {
            return Err(EditError::Preservation);
        }
        let mut values = Vec::new();
        reserve(&mut values, checked.schema().columns().len(), reader)?;
        values.resize_with(checked.schema().columns().len(), || {
            tablecraft_core::values::Value::Null
        });
        for (column, value) in columns.iter().zip(selected) {
            reader.tick()?;
            values[*column] = value;
        }
        let nulls = selection.profile.row_nulls(&values, reader)?;
        let wire = selection.profile.request(&values, *reference, reader)?;
        if selection.profile.variable() {
            if selection.profile.unique && !selection.profile.exact_unique(&wire) {
                return Err(EditError::Preservation);
            }
            facts.push(RowFacts {
                reference: *reference,
                nulls,
            });
        }
        if !selection.profile.ignore_nulls || !nulls.all {
            reader.work(size_of::<Wire>())?;
            expected.push(wire);
        }
    }
    super::wire::sort_paid(&mut expected, reader)?;
    for entry in &expected {
        reader.work(size_of::<Wire>().max(entry.len()))?;
    }
    if expected != entries {
        return Err(EditError::Preservation);
    }
    Ok(facts)
}
