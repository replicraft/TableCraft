//! Re-derive visible placement, allocation purposes and original map effects.
use super::{ImageProof, Row, reserve};
use crate::{
    EditError, InspectError, TableSchema,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapRole, RepresentedMap},
        bytes::{u16_at, u32_at},
        definition,
        layout::TABLE_FREE_MAP_OFFSET,
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{Reader, row_count},
        row::{
            DELETED_MASK, MAX_SLOTS, OFFSET_MASK, ROW_COUNT_OFFSET, SLOT_BYTES, UNSUPPORTED_MASK,
            reference, slot_offset,
        },
    },
    table::mutation::{
        insert::placement::Plan,
        variable_split::metadata::Metadata,
        variable_update::map_audit::{ExpectedByte, expect, hint},
    },
};
use std::io::Cursor;

const FREE_OFFSET: usize = DATA_SIGNATURE.len();

struct State {
    number: u32,
    count: usize,
    end: usize,
    hinted: bool,
}

struct Replay {
    states: Vec<State>,
    visited: Vec<u32>,
    edits: Vec<ExpectedByte>,
    bitmap: Option<hint::ExpectedBitmap>,
}

pub(super) fn prove(
    proof: &mut ImageProof<'_>,
    schema: &TableSchema,
    metadata: &Metadata,
    rows: &mut [Row],
    staged: &Plan,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    let definition = definition::read(reader, schema.definition)?;
    let hint_reference = u32_at(&definition, TABLE_FREE_MAP_OFFSET)?;
    let global = RepresentedMap::read(reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    let owned = RepresentedMap::read(reader, schema.layout.usage, MapRole::Owned)?;
    let candidate = RepresentedMap::read(reader, hint_reference, MapRole::TableFreeSpace)?;
    global.require_allocated_storage(reader, &global)?;
    owned.require_allocated_storage(reader, &global)?;
    candidate.require_allocated_storage(reader, &global)?;
    global.require_allocated(reader, schema.definition)?;

    let original_owned = allocation::pages(reader, schema.layout.usage)?;
    let mut hint = hint::Hint::capture(schema.definition, hint_reference, metadata, reader)?;
    if hint.is_some() {
        let members = allocation::pages(reader, hint_reference)?;
        for member in members {
            reader.work(original_owned.len())?;
            if original_owned.binary_search(&member).is_err() {
                return Err(EditError::Preservation);
            }
        }
    }
    let mut replay = Replay {
        states: Vec::new(),
        visited: Vec::new(),
        edits: Vec::new(),
        bitmap: None,
    };
    reserve(
        &mut replay.states,
        original_owned.len() + rows.len(),
        reader,
    )?;
    for number in original_owned {
        reader.tick()?;
        global.require_allocated(reader, number)?;
        let raw = reader.page(number)?;
        if raw[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || u32_at(&raw, OWNER_OFFSET)? != schema.definition
        {
            return Err(EditError::Preservation);
        }
        let (count, end) = geometry(&raw, reader)?;
        let hinted = match candidate.cell(reader, number) {
            Ok(cell) => cell.before & cell.mask != 0,
            Err(InspectError::Unsupported) => false,
            Err(error) => return Err(error.into()),
        };
        replay.states.push(State {
            number,
            count,
            end,
            hinted,
        });
    }

    let original_eof = reader.header.page_count();
    let mut next_eof = original_eof;
    for row in rows.iter_mut() {
        reader.tick()?;
        reader.work(replay.states.len())?;
        let highest = replay.states.iter().rposition(|state| state.hinted);
        let mut destination = None;
        if let Some(position) = highest {
            let state = &mut replay.states[position];
            metadata.require_data_page(state.number, reader.budget, reader.cancelled)?;
            visit(&mut replay.visited, state.number, reader)?;
            let free = state
                .end
                .checked_sub(slot_offset(state.count))
                .ok_or(EditError::Preservation)?;
            if state.count < MAX_SLOTS
                && row
                    .bytes
                    .len()
                    .checked_add(SLOT_BYTES)
                    .is_some_and(|needed| needed <= free)
            {
                destination = Some(position);
            } else {
                state.hinted = false;
                if let Some(hint) = &mut hint {
                    hint.remove(state.number, &mut replay.edits, reader)?;
                } else {
                    let cell = if state.number < original_eof {
                        candidate.cell(reader, state.number)?
                    } else {
                        candidate.future_cell(reader, state.number)?
                    };
                    expect(&mut replay.edits, cell, false, reader)?;
                }
            }
        }

        let position = if let Some(position) = destination {
            position
        } else {
            let number = next_eof;
            metadata.require_data_page(number, reader.budget, reader.cancelled)?;
            let owned_cell = owned.future_cell(reader, number)?;
            let global_cell = global.future_cell(reader, number)?;
            if owned_cell.before & owned_cell.mask != 0
                || global_cell.before & global_cell.mask == 0
            {
                return Err(EditError::Preservation);
            }
            expect(&mut replay.edits, owned_cell, true, reader)?;
            expect(&mut replay.edits, global_cell, false, reader)?;
            if hint.is_none() {
                let hint_cell = candidate.future_cell(reader, number)?;
                if hint_cell.before & hint_cell.mask != 0 {
                    return Err(EditError::Preservation);
                }
                expect(&mut replay.edits, hint_cell, true, reader)?;
            }

            proof.append(number, reader)?;
            let start = number as usize * PAGE_BYTES;
            let raw = &mut proof.bytes[start..start + PAGE_BYTES];
            raw[..DATA_SIGNATURE.len()].copy_from_slice(&DATA_SIGNATURE);
            raw[OWNER_OFFSET..OWNER_OFFSET + size_of::<u32>()]
                .copy_from_slice(&schema.definition.to_le_bytes());
            raw[FREE_OFFSET..FREE_OFFSET + size_of::<u16>()]
                .copy_from_slice(&((PAGE_BYTES - slot_offset(0)) as u16).to_le_bytes());
            next_eof = next_eof
                .checked_add(1)
                .ok_or(InspectError::StructureLimit)?;
            if let Some(hint) = &mut hint
                && let Some(bitmap) =
                    hint.add(number, &mut next_eof, &mut replay.edits, &global, reader)?
            {
                proof.append(bitmap, reader)?;
            }

            visit(&mut replay.visited, number, reader)?;
            replay.states.push(State {
                number,
                count: 0,
                end: PAGE_BYTES,
                hinted: true,
            });
            replay.states.len() - 1
        };
        let state = &mut replay.states[position];
        let start = state
            .end
            .checked_sub(row.bytes.len())
            .filter(|start| *start >= slot_offset(state.count + 1))
            .ok_or(EditError::Preservation)?;
        let word = u16::try_from(start).map_err(|_| EditError::Preservation)?;
        if word & !OFFSET_MASK != 0 || state.count >= MAX_SLOTS {
            return Err(EditError::Preservation);
        }
        let base = state.number as usize * PAGE_BYTES;
        row.reference = reference(state.number, state.count as u8);
        row.range = base + start..base + state.end;
        reader.work(row.bytes.len() + 3 * size_of::<u16>())?;
        proof.bytes[row.range.clone()].copy_from_slice(&row.bytes);
        proof.bytes[base + slot_offset(state.count)..base + slot_offset(state.count + 1)]
            .copy_from_slice(&word.to_le_bytes());
        state.count += 1;
        state.end = start;
        proof.bytes[base + ROW_COUNT_OFFSET..base + ROW_COUNT_OFFSET + size_of::<u16>()]
            .copy_from_slice(&(state.count as u16).to_le_bytes());
        let free = state.end - slot_offset(state.count);
        proof.bytes[base + FREE_OFFSET..base + FREE_OFFSET + size_of::<u16>()]
            .copy_from_slice(&(free as u16).to_le_bytes());
    }

    replay.bitmap = hint.and_then(hint::Hint::into_metadata);
    if let Some(bitmap) = &replay.bitmap {
        let start = bitmap.number as usize * PAGE_BYTES;
        reader.work(PAGE_BYTES)?;
        proof.bytes[start..start + PAGE_BYTES].copy_from_slice(&bitmap.bytes);
    }
    for byte in &replay.edits {
        reader.tick()?;
        if proof.original.get(byte.offset) != Some(&byte.before) {
            return Err(EditError::Preservation);
        }
        *proof
            .bytes
            .get_mut(byte.offset)
            .ok_or(EditError::Preservation)? = byte.after;
    }

    compare(proof, schema, rows, staged, &replay, reader)
}

fn visit(
    visited: &mut Vec<u32>,
    number: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.work(visited.len())?;
    if !visited.contains(&number) {
        reserve(visited, 1, reader)?;
        visited.push(number);
    }

    Ok(())
}

fn geometry(
    page: &[u8],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(usize, usize), EditError> {
    let count = row_count(page)?;
    let slots_end = slot_offset(count);
    let mut end = PAGE_BYTES;
    for slot in 0..count {
        reader.tick()?;
        let word = u16_at(page, slot_offset(slot))?;
        let start = usize::from(word & OFFSET_MASK);
        if word & UNSUPPORTED_MASK != 0
            || start < slots_end
            || start > end
            || (start == end && word & DELETED_MASK == 0)
        {
            return Err(EditError::Preservation);
        }
        end = start;
    }
    if usize::from(u16_at(page, FREE_OFFSET)?) != end - slots_end {
        return Err(EditError::Preservation);
    }

    Ok((count, end))
}

fn compare(
    proof: &ImageProof<'_>,
    schema: &TableSchema,
    rows: &[Row],
    staged: &Plan,
    replay: &Replay,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    if staged.append.original_length != proof.original.len()
        || staged.append.owner != schema.definition
        || staged.append.length != proof.bytes.len()
        || staged.append.reservations_len()
            != (proof.bytes.len() - proof.original.len()) / PAGE_BYTES
        || staged.rows.len() != rows.len()
        || staged.pages.len() != replay.visited.len()
        || staged.maps.owned_pages.len() != replay.states.len()
        || staged.append.map_edits.len() != replay.edits.len()
    {
        return Err(EditError::Preservation);
    }
    for (actual, row) in staged.rows.iter().zip(rows) {
        reader.tick()?;
        if actual.reference != row.reference || actual.range != row.range {
            return Err(EditError::Preservation);
        }
    }
    for (page, number) in staged.pages.iter().zip(&replay.visited) {
        reader.work(PAGE_BYTES)?;
        let start = *number as usize * PAGE_BYTES;
        let original = if start < proof.original.len() {
            &proof.original[start..start + PAGE_BYTES]
        } else {
            &[]
        };
        if page.number != *number
            || page.before != original
            || page.after != proof.bytes[start..start + PAGE_BYTES]
        {
            return Err(EditError::Preservation);
        }
    }
    for (number, state) in staged.maps.owned_pages.iter().zip(&replay.states) {
        reader.tick()?;
        if *number != state.number {
            return Err(EditError::Preservation);
        }
        if *number >= reader.header.page_count() {
            staged
                .append
                .require_data(*number, reader.budget, reader.cancelled)?;
        }
    }
    let mut actual_metadata = staged.maps.metadata_pages();
    if let Some(bitmap) = &replay.bitmap {
        let actual = actual_metadata.next().ok_or(EditError::Preservation)?;
        reader.work(PAGE_BYTES)?;
        if actual.0 != bitmap.number || actual.1 != bitmap.reference || actual.2 != bitmap.bytes {
            return Err(EditError::Preservation);
        }
        staged.append.require_candidate_bitmap(
            bitmap.number,
            schema.definition,
            bitmap.reference,
            reader.budget,
            reader.cancelled,
        )?;
    }
    if actual_metadata.next().is_some() {
        return Err(EditError::Preservation);
    }
    for byte in &replay.edits {
        reader.work(staged.append.map_edits.len())?;
        let actual = staged
            .append
            .map_edits
            .iter()
            .find(|actual| actual.offset == byte.offset)
            .ok_or(EditError::Preservation)?;
        if actual.before != byte.before || actual.after != byte.after {
            return Err(EditError::Preservation);
        }
    }

    Ok(())
}
