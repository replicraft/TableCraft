//! Exact represented index-owned/global effects; table hints are never authority.
use crate::{
    EditError, InspectError, NativeImage, NativeIndex,
    format::{
        allocation::{self, GLOBAL_FREE_REFERENCE, MapCell, MapRole, RepresentedMap},
        reader::Reader,
    },
    table::mutation::append,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

pub(in crate::table) use crate::table::mutation::append::Edit;

pub(super) fn allocate_nonroot(
    image: &NativeImage,
    index: &NativeIndex,
    number: u32,
    append: &mut append::State,
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
    if owned_cell.before & owned_cell.mask != 0
        || !append.global_is_free(&global, number, &mut reader)?
    {
        return Err(InspectError::Malformed.into());
    }
    change(&mut append.map_edits, owned_cell, true, &mut reader)?;
    append.claim_global(&global, number, &mut reader)?;
    Ok(())
}

pub(super) fn change(
    edits: &mut Vec<Edit>,
    cell: MapCell,
    set: bool,
    reader: &mut Reader<'_, Cursor<&[u8]>>,
) -> Result<(), EditError> {
    append::merge(edits, cell, set, reader)
}
