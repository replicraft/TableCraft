//! Exact represented index-owned/global effects; table hints are never authority.
use crate::{
    EditError, InspectError, NativeImage, NativeIndex,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapCell, MapRole, RepresentedMap},
        reader::Reader,
    },
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

pub(in crate::table) struct Edit {
    pub offset: usize,
    pub before: u8,
    pub after: u8,
}

pub(super) fn allocate_nonroot(
    image: &NativeImage,
    index: &NativeIndex,
    number: u32,
    edits: &mut Vec<Edit>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    global.require_allocated_storage(&mut reader, &global)?;
    let owned = RepresentedMap::read(&mut reader, index.usage_map(), MapRole::Owned)?;
    owned.require_allocated_storage(&mut reader, &global)?;
    for page in allocation::pages(&mut reader, index.usage_map())? {
        global.require_allocated(&mut reader, page)?;
    }
    let owned_cell = owned.future_cell(&mut reader, number)?;
    let free_cell = global.future_cell(&mut reader, number)?;
    if owned_cell.before & owned_cell.mask != 0 || free_cell.before & free_cell.mask == 0 {
        return Err(InspectError::Malformed.into());
    }
    change(edits, owned_cell, true, &mut reader)?;
    change(edits, free_cell, false, &mut reader)?;
    Ok(())
}

pub(super) fn change(
    edits: &mut Vec<Edit>,
    cell: MapCell,
    set: bool,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.work(edits.len())?;
    if let Some(edit) = edits.iter_mut().find(|edit| edit.offset == cell.offset) {
        if edit.before != cell.before {
            return Err(EditError::Preservation);
        }
        edit.after = if set {
            edit.after | cell.mask
        } else {
            edit.after & !cell.mask
        };
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
        });
    }
    Ok(())
}
