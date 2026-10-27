//! Original-before/current-after bytes; role-specific admission stays at callers.
use crate::{
    EditError, InspectError,
    format::{allocation::MapCell, reader::Reader},
    header::check,
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(in crate::table) struct Edit {
    pub offset: usize,
    pub before: u8,
    pub after: u8,
    touched: u8,
}

pub(in crate::table) fn current(
    edits: &[Edit],
    cell: &MapCell,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<bool, EditError> {
    reader.work(edits.len())?;
    let value = edits
        .iter()
        .find(|edit| edit.offset == cell.offset)
        .map_or(cell.before, |edit| edit.after);
    Ok(value & cell.mask != 0)
}

pub(in crate::table) fn claim(
    edits: &mut Vec<Edit>,
    cell: MapCell,
    set: bool,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    change(edits, cell, set, Collision::Untouched, reader)
}

pub(in crate::table) fn merge(
    edits: &mut Vec<Edit>,
    cell: MapCell,
    set: bool,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    change(edits, cell, set, Collision::Existing, reader)
}

enum Collision {
    Untouched,
    Existing,
}

fn change(
    edits: &mut Vec<Edit>,
    cell: MapCell,
    set: bool,
    collision: Collision,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.work(edits.len())?;
    if let Some(edit) = edits.iter_mut().find(|edit| edit.offset == cell.offset) {
        if edit.before != cell.before
            || matches!(collision, Collision::Untouched) && edit.touched & cell.mask != 0
        {
            return Err(EditError::Preservation);
        }
        edit.after = if set {
            edit.after | cell.mask
        } else {
            edit.after & !cell.mask
        };
        edit.touched |= cell.mask;
    } else {
        reader
            .budget
            .charge_allocation(1, size_of::<Edit>() as u64)
            .map_err(InspectError::from)?;
        edits.try_reserve(1).map_err(|_| InspectError::Allocation)?;
        edits.push(Edit {
            offset: cell.offset,
            before: cell.before,
            after: if set {
                cell.before | cell.mask
            } else {
                cell.before & !cell.mask
            },
            touched: cell.mask,
        });
    }

    Ok(())
}

/// A provisionally hinted DATA page may legitimately become full later.
pub(in crate::table) fn clear_hint(
    edits: &mut Vec<Edit>,
    cell: MapCell,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.work(edits.len())?;
    if let Some(edit) = edits.iter_mut().find(|edit| edit.offset == cell.offset) {
        if edit.before != cell.before {
            return Err(EditError::Preservation);
        }

        if edit.touched & cell.mask != 0 {
            edit.after &= !cell.mask;
            return Ok(());
        }
    }

    claim(edits, cell, false, reader)
}

pub(in crate::table) fn audit_bytes(
    edits: &[Edit],
    before: &[u8],
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    for edit in edits {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, 1)
            .map_err(InspectError::from)?;
        if before.get(edit.offset) != Some(&edit.before)
            || after.get(edit.offset) != Some(&edit.after)
            || (edit.before ^ edit.after) & !edit.touched != 0
        {
            return Err(EditError::Preservation);
        }
    }

    Ok(())
}
