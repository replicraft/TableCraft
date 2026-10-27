//! Original-before/current-after bytes; role-specific admission stays at callers.
use crate::{
    EditError, InspectError,
    format::{allocation::MapCell, page::PAGE_BYTES, reader::Reader},
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

/// Replace one caller-authorized original record, absorbing its earlier cells.
/// The caller owns the canonical role/range proof and complete after framing.
pub(in crate::table) fn rewrite_record(
    edits: &mut Vec<Edit>,
    offset: usize,
    before: &[u8],
    after: &[u8],
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.tick()?;
    if before.is_empty() || before.len() > PAGE_BYTES || before.len() != after.len() {
        return Err(EditError::Preservation);
    }
    let end = offset
        .checked_add(before.len())
        .ok_or(EditError::Preservation)?;

    reader.work(edits.len())?;
    for edit in edits.iter() {
        reader.tick()?;
        if (offset..end).contains(&edit.offset) && before[edit.offset - offset] != edit.before {
            return Err(EditError::Preservation);
        }
    }

    reader
        .budget
        .charge_allocation(before.len() as u64, size_of::<Edit>() as u64)
        .map_err(InspectError::from)?;
    edits
        .try_reserve(before.len())
        .map_err(|_| InspectError::Allocation)?;
    reader.work(before.len().saturating_mul(2))?;
    for _ in before {
        // No fallible work remains once any prior cell is retired.
        reader.tick()?;
    }

    edits.retain(|edit| !(offset..end).contains(&edit.offset));
    for (position, (&before, &after)) in before.iter().zip(after).enumerate() {
        edits.push(Edit {
            offset: offset + position,
            before,
            after,
            touched: u8::MAX,
        });
    }

    Ok(())
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
