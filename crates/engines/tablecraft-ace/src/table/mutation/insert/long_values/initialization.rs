//! Explicit empty declaration binding precedes new column-owned page allocation.
#[cfg(test)]
mod source_tests;
use super::{Initialization, MAP_CANDIDATE_SLOT, MAP_OWNED_SLOT, packed_maps};
use crate::{
    EditError, TableSchema,
    format::{allocation::RepresentedMap, reader::Reader},
    table::mutation::{
        append::{self, State},
        long_value::ownership::ZeroPair,
    },
};
use std::io::Cursor;

pub(super) fn prepare(
    schema: &TableSchema,
    zero: &ZeroPair,
    global: &RepresentedMap,
    append: &mut State,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Initialization, EditError> {
    reader.tick()?;
    if append.owner != schema.definition_page() {
        return Err(EditError::Preservation);
    }
    let number = append.next()?;
    let references = packed_maps(number)?;
    let cell = global.future_cell(reader, number)?;
    if !append::current(&append.map_edits, &cell, reader)? {
        return Err(EditError::Preservation);
    }
    append.reserve_long_value_map_container(
        number,
        zero.column,
        reader.budget,
        reader.cancelled,
    )?;
    append::claim(&mut append.map_edits, cell, false, reader)?;
    let mut after = [0; 2 * size_of::<u32>()];
    let pointer_bytes = size_of::<u32>();
    after[MAP_OWNED_SLOT * pointer_bytes..(MAP_OWNED_SLOT + 1) * pointer_bytes]
        .copy_from_slice(&references.owned().to_le_bytes());
    after[MAP_CANDIDATE_SLOT * pointer_bytes..(MAP_CANDIDATE_SLOT + 1) * pointer_bytes]
        .copy_from_slice(&references.candidate().to_le_bytes());
    Ok(Initialization {
        number,
        column: zero.column,
        offset: zero.offset,
        before: [0; 2 * size_of::<u32>()],
        after,
        bytes: Vec::new(),
    })
}
