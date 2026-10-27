//! Original-state replay derives map changes from real append capacity failures.
use super::super::EditError;
use super::{
    admission::Prepared,
    maps::{Family, Maps, Scope},
    placement::Plan,
};
use crate::{
    InspectError, NativeImage,
    format::{
        allocation::{self, MapCell, RepresentedMap},
        page::PAGE_BYTES,
        reader::{Reader, row_count, row_range},
        row::{MAX_SLOTS, SLOT_BYTES, reference_page, reference_slot, slot_offset},
    },
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

struct State {
    family: Family,
    page: u32,
    count: usize,
    end: usize,
    hinted: bool,
}

struct Expected {
    offset: usize,
    before: u8,
    after: u8,
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

fn expect(
    expected: &mut Vec<Expected>,
    cell: MapCell,
    set: bool,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.work(expected.len())?;
    if let Some(byte) = expected.iter_mut().find(|byte| byte.offset == cell.offset) {
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
            .charge_allocation(1, size_of::<Expected>() as u64)
            .map_err(InspectError::from)?;
        expected
            .try_reserve(1)
            .map_err(|_| InspectError::Allocation)?;
        expected.push(Expected {
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

fn initial(
    states: &mut Vec<State>,
    scope: &Scope,
    family: Family,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    for &number in &scope.pages {
        let page = reader.page(number)?;
        let count = row_count(&page)?;
        let end = if count == 0 {
            PAGE_BYTES
        } else {
            row_range(&page, count - 1)?.1
        };
        let hinted = match scope.hint.cell(reader, number) {
            Ok(cell) => cell.before & cell.mask != 0,
            Err(InspectError::Unsupported) => false,
            Err(error) => return Err(error.into()),
        };
        states.push(State {
            family,
            page: number,
            count,
            end,
            hinted,
        });
    }

    Ok(())
}

pub(super) fn prove(
    image: &NativeImage,
    prepared: &Prepared,
    plan: &Plan,
    after: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let schema = prepared.structural.schema();
    let number = schema.columns()[prepared.column].number();
    let references = prepared
        .storage
        .columns()
        .iter()
        .find(|column| column.column_number() == number)
        .and_then(|column| column.maps())
        .ok_or(EditError::Preservation)?;
    let original = Maps::capture(image, schema, references, budget, cancelled)?;
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let capacity = original
        .ordinary
        .pages
        .len()
        .checked_add(original.long_value.pages.len())
        .and_then(|count| count.checked_add(plan.appended.len()))
        .ok_or(InspectError::StructureLimit)?;
    reader
        .budget
        .charge_allocation(capacity as u64, size_of::<State>() as u64)
        .map_err(InspectError::from)?;
    let mut states = Vec::new();
    states
        .try_reserve_exact(capacity)
        .map_err(|_| InspectError::Allocation)?;
    initial(
        &mut states,
        &original.ordinary,
        Family::Ordinary,
        &mut reader,
    )?;
    initial(
        &mut states,
        &original.long_value,
        Family::LongValue,
        &mut reader,
    )?;
    let mut expected = Vec::new();
    let mut next_eof = original.original_eof;
    let mut attempts = 0;

    for event in &plan.appended {
        reader.tick()?;
        let length = event
            .range
            .end
            .checked_sub(event.range.start)
            .filter(|length| *length > 0)
            .ok_or(EditError::Preservation)?;
        let scope = match event.family {
            Family::Ordinary => &original.ordinary,
            Family::LongValue => &original.long_value,
        };
        let mut destination = None;
        loop {
            reader.work(states.len())?;
            let mut candidate = None;
            for (position, state) in states.iter().enumerate() {
                reader.tick()?;
                if state.family == event.family
                    && state.hinted
                    && candidate.is_none_or(|previous: usize| states[previous].page < state.page)
                {
                    candidate = Some(position);
                }
            }
            let Some(position) = candidate else {
                break;
            };
            let state = &mut states[position];
            let free = state
                .end
                .checked_sub(slot_offset(state.count))
                .ok_or(EditError::Preservation)?;
            let fitted = state.count < MAX_SLOTS
                && length
                    .checked_add(SLOT_BYTES)
                    .is_some_and(|need| need <= free);
            let attempt = plan.attempts.get(attempts).ok_or(EditError::Preservation)?;
            if attempt.family != event.family
                || attempt.page != state.page
                || attempt.bytes != length
                || attempt.fitted != fitted
            {
                return Err(EditError::Preservation);
            }
            attempts += 1;
            if fitted {
                destination = Some(position);
                break;
            }

            state.hinted = false;
            let hint = cell(&scope.hint, state.page, &mut reader)?;
            expect(&mut expected, hint, false, &mut reader)?;
        }
        let position = if let Some(position) = destination {
            position
        } else {
            let owned = cell(&scope.owned, next_eof, &mut reader)?;
            let hint = cell(&scope.hint, next_eof, &mut reader)?;
            let global = cell(&original.global, next_eof, &mut reader)?;
            if owned.before & owned.mask != 0
                || hint.before & hint.mask != 0
                || global.before & global.mask == 0
            {
                return Err(EditError::Preservation);
            }
            expect(&mut expected, owned, true, &mut reader)?;
            expect(&mut expected, hint, true, &mut reader)?;
            expect(&mut expected, global, false, &mut reader)?;
            states.push(State {
                family: event.family,
                page: next_eof,
                count: 0,
                end: PAGE_BYTES,
                hinted: true,
            });
            next_eof = next_eof.checked_add(1).ok_or(EditError::Preservation)?;
            states.len() - 1
        };
        let state = &mut states[position];
        let start = state
            .end
            .checked_sub(length)
            .ok_or(EditError::Preservation)?;
        if state.count >= MAX_SLOTS
            || start < slot_offset(state.count + 1)
            || reference_page(event.reference) != state.page
            || usize::from(reference_slot(event.reference)) != state.count
            || event.range.start != state.page as usize * PAGE_BYTES + start
            || event.range.end != state.page as usize * PAGE_BYTES + state.end
        {
            return Err(EditError::Preservation);
        }
        state.count += 1;
        state.end = start;
    }
    if attempts != plan.attempts.len()
        || next_eof as usize * PAGE_BYTES != plan.length
        || expected.len() != plan.maps.edits.len()
    {
        return Err(EditError::Preservation);
    }
    for byte in expected {
        reader.tick()?;
        reader.work(plan.maps.edits.len())?;
        let edit = plan
            .maps
            .edits
            .iter()
            .find(|edit| edit.offset == byte.offset)
            .ok_or(EditError::Preservation)?;
        if edit.before != byte.before
            || edit.after != byte.after
            || image.as_bytes().get(byte.offset) != Some(&byte.before)
            || after.as_bytes().get(byte.offset) != Some(&byte.after)
        {
            return Err(EditError::Preservation);
        }
    }

    let mut source = after.reader();
    let mut final_reader = Reader {
        source: &mut source,
        header: after.catalog().header(),
        budget: reader.budget,
        cancelled,
        reads: 0,
    };
    for (family, owned_reference, hint_reference) in [
        (
            Family::Ordinary,
            schema.layout.usage,
            original.ordinary.hint_reference,
        ),
        (
            Family::LongValue,
            references.owned(),
            references.candidate(),
        ),
    ] {
        let owned = allocation::pages(&mut final_reader, owned_reference)?;
        let hints = allocation::pages(&mut final_reader, hint_reference)?;
        final_reader.work(states.len() + owned.len() + hints.len())?;
        if owned.len() != states.iter().filter(|state| state.family == family).count()
            || hints.len()
                != states
                    .iter()
                    .filter(|state| state.family == family && state.hinted)
                    .count()
        {
            return Err(EditError::Preservation);
        }
        for state in states.iter().filter(|state| state.family == family) {
            final_reader.tick()?;
            final_reader.work(owned.len() + hints.len())?;
            if !owned.contains(&state.page) || hints.contains(&state.page) != state.hinted {
                return Err(EditError::Preservation);
            }
        }
    }

    Ok(())
}
