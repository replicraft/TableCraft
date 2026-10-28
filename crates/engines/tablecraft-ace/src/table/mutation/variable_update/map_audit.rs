//! Independent placement replay derives map authority without planner masks.
mod certified;
pub(in crate::table::mutation) mod hint;
#[cfg(test)]
mod tests;

pub(in crate::table) use certified::CertifiedData;
pub(in crate::table::mutation) use certified::certify;

use super::super::EditError;
use super::super::variable_split::metadata::Metadata;
use super::authority::ChangedRecord;
use super::placement::{DataView, Plan};
use crate::{
    InspectError, NativeImage, TableSchema,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapCell, MapRole, RepresentedMap},
        bytes::{u16_at, u32_at},
        definition,
        layout::TABLE_FREE_MAP_OFFSET,
        page::{DATA_SIGNATURE, OWNER_OFFSET, PAGE_BYTES},
        reader::{Reader, row_count, row_range},
        row::{MAX_SLOTS, SLOT_BYTES, reference_page, slot_offset},
    },
    header::check,
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_OFFSET: usize = DATA_SIGNATURE.len();

struct State {
    number: u32,
    count: usize,
    end: usize,
    hinted: bool,
}

pub(in crate::table::mutation) struct ExpectedByte {
    pub(in crate::table::mutation) offset: usize,
    pub(in crate::table::mutation) before: u8,
    pub(in crate::table::mutation) after: u8,
}

struct Replay {
    end: u32,
    edits: Vec<ExpectedByte>,
    data_pages: Vec<u32>,
    phase_pages: Vec<u32>,
    metadata: Option<hint::ExpectedBitmap>,
}

pub(in crate::table::mutation) fn expect(
    bytes: &mut Vec<ExpectedByte>,
    cell: MapCell,
    set: bool,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.work(bytes.len())?;
    if let Some(byte) = bytes.iter_mut().find(|byte| byte.offset == cell.offset) {
        if byte.before != cell.before {
            return Err(EditError::Preservation);
        }
        byte.after = if set {
            byte.after | cell.mask
        } else {
            byte.after & !cell.mask
        };
    } else {
        reader
            .budget
            .charge_allocation(1, size_of::<ExpectedByte>() as u64)
            .map_err(InspectError::from)?;
        bytes.try_reserve(1).map_err(|_| InspectError::Allocation)?;
        bytes.push(ExpectedByte {
            offset: cell.offset,
            before: cell.before,
            after: if set {
                cell.before | cell.mask
            } else {
                cell.before & !cell.mask
            },
        });
    }
    Ok(())
}

fn cell(
    map: &RepresentedMap,
    number: u32,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<MapCell, EditError> {
    if number < reader.header.page_count() {
        Ok(map.cell(reader, number)?)
    } else {
        Ok(map.future_cell(reader, number)?)
    }
}

pub(super) fn prove(
    image: &NativeImage,
    schema: &TableSchema,
    plan: &Plan,
    records: &[ChangedRecord],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let replay = replay(image, schema, plan.view(), records, None, budget, cancelled)?;
    compare(&replay.edits, plan, budget, cancelled)
}

fn replay(
    image: &NativeImage,
    schema: &TableSchema,
    plan: DataView<'_>,
    records: &[ChangedRecord],
    metadata: Option<&Metadata>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Replay, EditError> {
    if plan.events.len() != records.len() || plan.append.owner != schema.definition {
        return Err(EditError::Preservation);
    }

    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let definition_bytes = definition::read(&mut reader, schema.definition)?;
    let hint_reference = u32_at(&definition_bytes, TABLE_FREE_MAP_OFFSET)?;
    let owned = RepresentedMap::read(&mut reader, schema.layout.usage, MapRole::Owned)?;
    let hint = RepresentedMap::read(&mut reader, hint_reference, MapRole::TableFreeSpace)?;
    let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    let original = allocation::pages(&mut reader, schema.layout.usage)?;
    reader
        .budget
        .charge_allocation(
            (original.len() + plan.events.len()) as u64,
            size_of::<State>() as u64,
        )
        .map_err(InspectError::from)?;
    let mut states = Vec::new();
    states
        .try_reserve_exact(original.len() + plan.events.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut expected = Vec::new();
    let mut data_pages = Vec::new();
    let mut phase_pages = Vec::new();
    let mut hint_replay = if let Some(metadata) = metadata {
        hint::Hint::capture(schema.definition, hint_reference, metadata, &mut reader)?
    } else {
        None
    };

    for number in original {
        reader.tick()?;
        let page = reader.page(number)?;
        if page[..DATA_SIGNATURE.len()] != DATA_SIGNATURE
            || u32_at(&page, OWNER_OFFSET)? != schema.definition
        {
            return Err(EditError::Preservation);
        }
        let count = row_count(&page)?;
        let end = if count == 0 {
            PAGE_BYTES
        } else {
            row_range(&page, count - 1)?.1
        };
        if usize::from(u16_at(&page, FREE_OFFSET)?) != end - slot_offset(count) {
            return Err(EditError::Preservation);
        }
        let hinted = match hint.cell(&mut reader, number) {
            Ok(cell) => cell.before & cell.mask != 0,
            Err(InspectError::Unsupported) => false,
            Err(error) => return Err(error.into()),
        };
        states.push(State {
            number,
            count,
            end,
            hinted,
        });
    }
    let original_eof = reader.header.page_count();
    let mut next_eof = original_eof;
    for (event, record) in plan.events.iter().zip(records) {
        reader.tick()?;
        if let Some(metadata) = metadata {
            metadata.require_data_page(
                reference_page(record.reference),
                reader.budget,
                cancelled,
            )?;
            let final_page = u32::try_from(record.final_offset / PAGE_BYTES)
                .map_err(|_| EditError::Preservation)?;
            metadata.require_data_page(final_page, reader.budget, cancelled)?;
        }
        if event.root != record.reference
            || event.word != record.root_word
            || event.root_range != record.root_range
        {
            return Err(EditError::Preservation);
        }

        // Record capacity and preferred wire bytes come from the original
        // request proof, never the builder's selected destination range.
        let length = record.bytes.len();
        if length == record.original_length {
            let end = record
                .final_offset
                .checked_add(length)
                .ok_or(EditError::Preservation)?;
            if event.appended.is_some() || event.target != (record.final_offset..end) {
                return Err(EditError::Preservation);
            }

            continue;
        }

        if length < record.original_length {
            return Err(EditError::Preservation);
        }

        let slot = event.appended.ok_or(EditError::Preservation)?;
        let number =
            u32::try_from(event.target.start / PAGE_BYTES).map_err(|_| EditError::Preservation)?;
        reader.work(states.len())?;
        let highest = states.iter().rposition(|state| state.hinted);
        let mut destination = None;
        if let Some(position) = highest {
            let state = &mut states[position];
            if let Some(metadata) = metadata {
                // A metadata-bearing highest hint is a refusal, never a
                // capacity failure which grants permission to clear its bit.
                metadata.require_data_page(state.number, reader.budget, cancelled)?;
            }
            let free = state
                .end
                .checked_sub(slot_offset(state.count))
                .ok_or(EditError::Preservation)?;
            if state.count < MAX_SLOTS
                && length
                    .checked_add(SLOT_BYTES)
                    .is_some_and(|required| required <= free)
            {
                if number != state.number {
                    return Err(EditError::Preservation);
                }
                destination = Some(position);
            } else {
                // Only this actual highest candidate and this requested length
                // can authorize a hint clear. A later forged failed_fit call
                // cannot invent an event or consume an unrelated hint bit.
                state.hinted = false;
                if let Some(replay) = &mut hint_replay {
                    replay.remove(state.number, &mut expected, &mut reader)?;
                } else {
                    expect(
                        &mut expected,
                        cell(&hint, state.number, &mut reader)?,
                        false,
                        &mut reader,
                    )?;
                }
            }
        }
        let position = if let Some(position) = destination {
            position
        } else {
            if number != next_eof {
                return Err(EditError::Preservation);
            }
            let owned_cell = owned.future_cell(&mut reader, number)?;
            let hint_cell = if hint_replay.is_none() {
                Some(hint.future_cell(&mut reader, number)?)
            } else {
                None
            };
            let global_cell = global.future_cell(&mut reader, number)?;
            if owned_cell.before & owned_cell.mask != 0
                || global_cell.before & global_cell.mask == 0
            {
                return Err(EditError::Preservation);
            }
            if let Some(hint_cell) = &hint_cell
                && hint_cell.before & hint_cell.mask != 0
            {
                return Err(EditError::Preservation);
            }
            expect(&mut expected, owned_cell, true, &mut reader)?;
            if let Some(hint_cell) = hint_cell {
                expect(&mut expected, hint_cell, true, &mut reader)?;
            }
            expect(&mut expected, global_cell, false, &mut reader)?;
            states.push(State {
                number,
                count: 0,
                end: PAGE_BYTES,
                hinted: true,
            });
            reader
                .budget
                .charge_allocation(1, size_of::<u32>() as u64)
                .map_err(InspectError::from)?;
            data_pages
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            data_pages.push(number);
            reader
                .budget
                .charge_allocation(1, size_of::<u32>() as u64)
                .map_err(InspectError::from)?;
            phase_pages
                .try_reserve(1)
                .map_err(|_| InspectError::Allocation)?;
            phase_pages.push(number);
            next_eof = next_eof.checked_add(1).ok_or(EditError::Preservation)?;
            if let Some(replay) = &mut hint_replay
                && let Some(bitmap) =
                    replay.add(number, &mut next_eof, &mut expected, &global, &mut reader)?
            {
                reader
                    .budget
                    .charge_allocation(1, size_of::<u32>() as u64)
                    .map_err(InspectError::from)?;
                phase_pages
                    .try_reserve(1)
                    .map_err(|_| InspectError::Allocation)?;
                phase_pages.push(bitmap);
            }
            states.len() - 1
        };
        let state = &mut states[position];
        let start = state
            .end
            .checked_sub(length)
            .filter(|&start| start >= slot_offset(state.count + 1))
            .ok_or(EditError::Preservation)?;
        if usize::from(slot) != state.count
            || event.target
                != (number as usize * PAGE_BYTES + start..number as usize * PAGE_BYTES + state.end)
        {
            return Err(EditError::Preservation);
        }
        state.count += 1;
        state.end = start;
    }
    if !plan.length.is_multiple_of(PAGE_BYTES)
        || plan.length / PAGE_BYTES != next_eof as usize
        || states.len() != plan.maps.owned_pages.len()
    {
        return Err(EditError::Preservation);
    }
    for (state, actual) in states.iter().zip(&plan.maps.owned_pages) {
        reader.tick()?;
        if state.number != *actual {
            return Err(EditError::Preservation);
        }
    }
    check(reader.budget, reader.cancelled)?;
    Ok(Replay {
        end: next_eof,
        edits: expected,
        data_pages,
        phase_pages,
        metadata: hint_replay.and_then(hint::Hint::into_metadata),
    })
}

fn compare(
    expected: &[ExpectedByte],
    plan: &Plan,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    if expected.len() != plan.append.map_edits.len() {
        return Err(EditError::Preservation);
    }
    for byte in expected {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, plan.append.map_edits.len() as u64)
            .map_err(InspectError::from)?;
        let actual = plan
            .append
            .map_edits
            .iter()
            .find(|edit| edit.offset == byte.offset)
            .ok_or(EditError::Preservation)?;
        if actual.before != byte.before || actual.after != byte.after {
            return Err(EditError::Preservation);
        }
    }
    // Header/slot geometry is also independently checked against complete page
    // bytes. This replay only grants exact map transitions, never page contents.
    check(budget, cancelled)?;
    Ok(())
}
