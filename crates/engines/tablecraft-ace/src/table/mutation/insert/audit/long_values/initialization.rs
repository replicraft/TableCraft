//! New map containers and declaration pointers are derived from source zero pairs.
use super::super::{ImageProof, reserve};
use crate::{
    EditError, InspectError, LongValueMapReferences, TableSchema,
    format::{allocation::RepresentedMap, page::PAGE_BYTES, reader::Reader, row::reference},
    table::mutation::{insert::Staged, long_value::ownership::ZeroPair},
};
use std::io::Cursor;

const MAP_BYTES: usize = 69;
const MAP_COUNT: usize = 2;
const DATA_SIGNATURE: [u8; 2] = [1, 1];
const MAX_PACKED_PAGE: u32 = u32::MAX >> u8::BITS;
const SLOT_BYTES: usize = size_of::<u16>();
const SLOT_START: usize = 14;
const POINTER_BYTES: usize = size_of::<u32>();
const POINTER_PAIR_BYTES: usize = 2 * POINTER_BYTES;
const COUNT_OFFSET: usize = 12;
const FREE_OFFSET: usize = 2;

pub(super) struct Initialization {
    pub position: usize,
    pub number: u32,
    pub column: usize,
    pub offset: usize,
    pub references: LongValueMapReferences,
    after: [u8; POINTER_PAIR_BYTES],
}

pub(super) fn derive(
    schema: &TableSchema,
    selected: &[usize],
    zeros: &[ZeroPair],
    global: &RepresentedMap,
    proof: &mut ImageProof<'_>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<Vec<Initialization>, EditError> {
    let mut ordered = Vec::new();
    reserve(&mut ordered, selected.len(), reader)?;

    for &position in selected {
        reader.work(zeros.len())?;
        if zeros
            .iter()
            .any(|zero| zero.column == schema.columns()[position].number())
        {
            ordered.push(position);
        }
    }

    let levels = ordered.len().checked_ilog2().unwrap_or(0) as usize + 1;
    let sort_work = ordered
        .len()
        .checked_mul(levels)
        .ok_or(InspectError::StructureLimit)?;
    reader.work(sort_work)?;
    ordered.sort_unstable_by_key(|position| schema.columns()[*position].field.variable);

    let mut result = Vec::new();
    reserve(&mut result, ordered.len(), reader)?;

    for position in ordered {
        reader.tick()?;
        let column = schema.columns()[position].number();
        reader.work(zeros.len())?;
        let zero = zeros
            .iter()
            .find(|zero| zero.column == column)
            .ok_or(EditError::Preservation)?;
        let end = zero
            .offset
            .checked_add(POINTER_PAIR_BYTES)
            .ok_or(InspectError::StructureLimit)?;
        if proof.original.get(zero.offset..end) != Some([0; POINTER_PAIR_BYTES].as_slice())
            || proof.bytes.get(zero.offset..end) != Some([0; POINTER_PAIR_BYTES].as_slice())
        {
            return Err(EditError::Preservation);
        }

        let number = u32::try_from(proof.bytes.len() / PAGE_BYTES)
            .map_err(|_| InspectError::StructureLimit)?;
        if number > MAX_PACKED_PAGE {
            return Err(InspectError::StructureLimit.into());
        }
        super::cursor::require_bit(
            proof,
            global,
            reader.header.page_count(),
            number,
            true,
            reader,
        )?;
        super::cursor::change(
            proof,
            global,
            reader.header.page_count(),
            number,
            false,
            reader,
        )?;
        proof.append(number, reader)?;
        let base = number as usize * PAGE_BYTES;
        let page = &mut proof.bytes[base..base + PAGE_BYTES];
        page[..DATA_SIGNATURE.len()].copy_from_slice(&DATA_SIGNATURE);
        let first = PAGE_BYTES - MAP_BYTES;
        let second = first - MAP_BYTES;
        let free = second - SLOT_START - MAP_COUNT * SLOT_BYTES;
        page[FREE_OFFSET..FREE_OFFSET + SLOT_BYTES].copy_from_slice(&(free as u16).to_le_bytes());
        page[COUNT_OFFSET..COUNT_OFFSET + SLOT_BYTES]
            .copy_from_slice(&(MAP_COUNT as u16).to_le_bytes());
        page[SLOT_START..SLOT_START + SLOT_BYTES].copy_from_slice(&(first as u16).to_le_bytes());
        page[SLOT_START + SLOT_BYTES..SLOT_START + MAP_COUNT * SLOT_BYTES]
            .copy_from_slice(&(second as u16).to_le_bytes());

        let owned = reference(number, 0);
        let candidate = reference(number, 1);
        let mut after = [0; POINTER_PAIR_BYTES];
        after[..POINTER_BYTES].copy_from_slice(&owned.to_le_bytes());
        after[POINTER_BYTES..].copy_from_slice(&candidate.to_le_bytes());
        reader.work(POINTER_PAIR_BYTES + SLOT_START + MAP_COUNT * SLOT_BYTES)?;
        proof.bytes[zero.offset..end].copy_from_slice(&after);

        result.push(Initialization {
            position,
            number,
            column,
            offset: zero.offset,
            references: LongValueMapReferences { owned, candidate },
            after,
        });
    }

    Ok(result)
}

pub(super) fn compare(
    expected: &[Initialization],
    staged: &Staged,
    proof: &ImageProof<'_>,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    if expected.len() != staged.long_values.initializations.len() {
        return Err(EditError::Preservation);
    }

    let mut purposes = staged.data.append.long_value_map_containers();
    for (expected, actual) in expected.iter().zip(&staged.long_values.initializations) {
        reader.work(PAGE_BYTES)?;
        let base = expected.number as usize * PAGE_BYTES;
        let purpose = purposes.next().ok_or(EditError::Preservation)?;
        if actual.number != expected.number
            || actual.column != expected.column
            || actual.offset != expected.offset
            || actual.before != [0; POINTER_PAIR_BYTES]
            || actual.after != expected.after
            || actual.bytes != proof.bytes[base..base + PAGE_BYTES]
            || purpose.number != expected.number
            || purpose.column != expected.column
            || purpose.definition != staged.data.append.owner
        {
            return Err(EditError::Preservation);
        }
        staged.data.append.require_long_value_map_container(
            expected.number,
            staged.data.append.owner,
            expected.column,
            reader.budget,
            reader.cancelled,
        )?;
    }

    if purposes.next().is_some() {
        return Err(EditError::Preservation);
    }
    Ok(())
}
