//! Explicit declaration bindings precede new column-owned page allocation.
#[cfg(test)]
mod source_tests;
use super::{
    DeclarationBinding, DefinitionGrowth, Initialization, MAP_CANDIDATE_SLOT, MAP_OWNED_SLOT,
    packed_maps,
};
use crate::{
    EditError, InspectError, NativeImage, TableSchema, WriteBlocker,
    format::{allocation::RepresentedMap, page::PAGE_BYTES, reader::Reader},
    table::{
        long_value::maps::DefinitionTail,
        mutation::{
            append::State,
            long_value::ownership::{MissingDeclaration, ZeroPair},
        },
    },
};
use std::io::Cursor;

const PAIR_BYTES: usize = size_of::<u16>() + 2 * size_of::<u32>();
const TRAILER: [u8; size_of::<u16>()] = u16::MAX.to_le_bytes();
const DEFINITION_HEADER_BYTES: usize = 2 * size_of::<u32>();

pub(super) fn prepare_zero(
    schema: &TableSchema,
    zero: &ZeroPair,
    global: &RepresentedMap,
    append: &mut State,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Initialization, EditError> {
    let number = allocate(schema, zero.column, global, append, reader)?;
    let references = packed_maps(number)?;
    let mut after = [0; 2 * size_of::<u32>()];
    let pointer_bytes = size_of::<u32>();
    after[MAP_OWNED_SLOT * pointer_bytes..(MAP_OWNED_SLOT + 1) * pointer_bytes]
        .copy_from_slice(&references.owned().to_le_bytes());
    after[MAP_CANDIDATE_SLOT * pointer_bytes..(MAP_CANDIDATE_SLOT + 1) * pointer_bytes]
        .copy_from_slice(&references.candidate().to_le_bytes());
    Ok(Initialization {
        number,
        column: zero.column,
        binding: DeclarationBinding::ZeroPair {
            offset: zero.offset,
            before: [0; 2 * size_of::<u32>()],
            after,
        },
        bytes: Vec::new(),
    })
}

pub(super) fn prepare_missing(
    schema: &TableSchema,
    missing: &MissingDeclaration,
    global: &RepresentedMap,
    append: &mut State,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Initialization, EditError> {
    let number = allocate(schema, missing.column, global, append, reader)?;
    Ok(Initialization {
        number,
        column: missing.column,
        binding: DeclarationBinding::MissingDeclaration,
        bytes: Vec::new(),
    })
}

fn allocate(
    schema: &TableSchema,
    column: usize,
    global: &RepresentedMap,
    append: &mut State,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<u32, EditError> {
    reader.tick()?;
    if append.owner != schema.definition_page() {
        return Err(EditError::Preservation);
    }
    let number = append.next()?;
    packed_maps(number)?;
    append.claim_global(global, number, reader)?;
    append.reserve_long_value_map_container(number, column, reader.budget, reader.cancelled)?;
    Ok(number)
}

pub(super) fn growth_length(tail: &DefinitionTail, count: usize) -> Result<usize, EditError> {
    count
        .checked_mul(PAIR_BYTES)
        .and_then(|bytes| tail.old_length.checked_add(bytes))
        .filter(|length| count != 0 && *length <= PAGE_BYTES)
        .ok_or(EditError::Unsupported(WriteBlocker::RowLayout))
}

pub(super) fn definition_growth(
    image: &NativeImage,
    tail: &DefinitionTail,
    initializations: &[Initialization],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<DefinitionGrowth, EditError> {
    reader.work(initializations.len())?;
    let count = initializations
        .iter()
        .filter(|initialization| {
            matches!(
                initialization.binding,
                DeclarationBinding::MissingDeclaration
            )
        })
        .count();
    let new_length = growth_length(tail, count)?;
    let bytes = new_length - tail.old_length + TRAILER.len();
    let end = tail
        .terminal
        .checked_add(bytes)
        .ok_or(InspectError::StructureLimit)?;
    let before = image
        .as_bytes()
        .get(tail.terminal..end)
        .ok_or(InspectError::Truncated)?;
    reader.work(bytes)?;
    let mut original = Vec::new();
    super::reserve(&mut original, bytes, reader.budget)?;
    original.extend_from_slice(before);
    let mut after = Vec::new();
    super::reserve(&mut after, bytes, reader.budget)?;
    for initialization in initializations {
        reader.tick()?;
        if !matches!(
            initialization.binding,
            DeclarationBinding::MissingDeclaration
        ) {
            continue;
        }
        let column =
            u16::try_from(initialization.column).map_err(|_| InspectError::StructureLimit)?;
        let references = packed_maps(initialization.number)?;
        after.extend_from_slice(&column.to_le_bytes());
        after.extend_from_slice(&references.owned().to_le_bytes());
        after.extend_from_slice(&references.candidate().to_le_bytes());
    }
    after.extend_from_slice(&TRAILER);
    if after.len() != bytes {
        return Err(EditError::Preservation);
    }
    Ok(DefinitionGrowth {
        definition: tail.definition,
        offset: tail.terminal,
        old_length: tail.old_length,
        new_length,
        old_free: tail.old_free,
        new_free: PAGE_BYTES.saturating_sub(new_length + DEFINITION_HEADER_BYTES) as u16,
        before: original,
        after,
    })
}
