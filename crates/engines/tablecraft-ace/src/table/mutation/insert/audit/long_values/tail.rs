//! Independently extend one certified TD terminal without replacing its counters.
use super::{
    super::{ImageProof, copied, reserve},
    initialization::Initialization,
};
use crate::{
    EditError, InspectError, TableSchema,
    format::{
        bytes::{u16_at, u32_at},
        page::PAGE_BYTES,
        reader::Reader,
    },
    table::{long_value::maps::DefinitionTail, mutation::insert::long_values::DefinitionGrowth},
};
use std::io::Cursor;

const FREE_OFFSET: usize = 2;
const NEXT_OFFSET: usize = 4;
const LENGTH_OFFSET: usize = 8;
const FREE_BYTES: usize = size_of::<u16>();
const LENGTH_BYTES: usize = size_of::<u32>();
const TRAILER: [u8; size_of::<u16>()] = [255; size_of::<u16>()];
const COLUMN_BYTES: usize = size_of::<u16>();
const PAIR_BYTES: usize = COLUMN_BYTES + 2 * size_of::<u32>();
const FREE_RESERVE: usize = 8;

pub(super) struct Growth {
    definition: u32,
    offset: usize,
    old_length: usize,
    new_length: usize,
    old_free: u16,
    new_free: u16,
    before: Vec<u8>,
    after: Vec<u8>,
}

pub(super) fn derive(
    schema: &TableSchema,
    source: Option<&DefinitionTail>,
    initializations: &[Initialization],
    proof: &mut ImageProof<'_>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Option<Growth>, EditError> {
    reader.work(initializations.len())?;
    let count = initializations
        .iter()
        .filter(|entry| entry.offset.is_none())
        .count();
    if count == 0 {
        return Ok(None);
    }
    let source = source.ok_or(EditError::Preservation)?;
    let extra = count
        .checked_mul(PAIR_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let new_length = source
        .old_length
        .checked_add(extra)
        .filter(|length| *length <= PAGE_BYTES)
        .ok_or(EditError::Preservation)?;
    let base = (schema.definition_page() as usize)
        .checked_mul(PAGE_BYTES)
        .ok_or(InspectError::StructureLimit)?;
    let original = proof
        .original
        .get(base..base + PAGE_BYTES)
        .ok_or(EditError::Preservation)?;
    if source.definition != schema.definition_page()
        || source.old_length < TRAILER.len()
        || source.terminal != base + source.old_length - TRAILER.len()
        || u32_at(original, NEXT_OFFSET)? != 0
        || u32_at(original, LENGTH_OFFSET)? as usize != source.old_length
        || u16_at(original, FREE_OFFSET)? != source.old_free
        || usize::from(source.old_free)
            != PAGE_BYTES.saturating_sub(source.old_length + FREE_RESERVE)
    {
        return Err(EditError::Preservation);
    }

    let span = extra
        .checked_add(TRAILER.len())
        .ok_or(InspectError::StructureLimit)?;
    let end = source
        .terminal
        .checked_add(span)
        .ok_or(InspectError::StructureLimit)?;
    let before = copied(
        proof
            .original
            .get(source.terminal..end)
            .ok_or(EditError::Preservation)?,
        reader,
    )?;
    if before[..TRAILER.len()] != TRAILER
        || proof.bytes.get(source.terminal..end) != Some(before.as_slice())
    {
        return Err(EditError::Preservation);
    }
    let mut after = Vec::new();
    reserve(&mut after, span, reader)?;
    reader.work(span)?;
    for entry in initializations {
        reader.tick()?;
        if entry.offset.is_none() {
            let column = u16::try_from(entry.column).map_err(|_| InspectError::StructureLimit)?;
            after.extend_from_slice(&column.to_le_bytes());
            after.extend_from_slice(&entry.after);
        }
    }
    after.extend_from_slice(&TRAILER);
    if after.len() != span {
        return Err(EditError::Preservation);
    }

    let new_free = u16::try_from(PAGE_BYTES.saturating_sub(new_length + FREE_RESERVE))
        .map_err(|_| InspectError::StructureLimit)?;
    reader.work(span + FREE_BYTES + LENGTH_BYTES)?;
    proof.bytes[source.terminal..end].copy_from_slice(&after);
    proof.bytes[base + FREE_OFFSET..base + FREE_OFFSET + FREE_BYTES]
        .copy_from_slice(&new_free.to_le_bytes());
    proof.bytes[base + LENGTH_OFFSET..base + LENGTH_OFFSET + LENGTH_BYTES]
        .copy_from_slice(&(new_length as u32).to_le_bytes());
    Ok(Some(Growth {
        definition: source.definition,
        offset: source.terminal,
        old_length: source.old_length,
        new_length,
        old_free: source.old_free,
        new_free,
        before,
        after,
    }))
}

pub(super) fn compare(
    expected: Option<&Growth>,
    actual: Option<&DefinitionGrowth>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    match (expected, actual) {
        (None, None) => Ok(()),
        (Some(expected), Some(actual)) => {
            let work = expected
                .before
                .len()
                .checked_mul(2)
                .ok_or(InspectError::StructureLimit)?;
            reader.work(work)?;
            if expected.definition != actual.definition
                || expected.offset != actual.offset
                || expected.old_length != actual.old_length
                || expected.new_length != actual.new_length
                || expected.old_free != actual.old_free
                || expected.new_free != actual.new_free
                || expected.before != actual.before
                || expected.after != actual.after
            {
                return Err(EditError::Preservation);
            }
            Ok(())
        }
        _ => Err(EditError::Preservation),
    }
}
