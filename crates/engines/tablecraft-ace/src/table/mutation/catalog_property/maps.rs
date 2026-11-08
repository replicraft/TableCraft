//! Derive the selected column's two distinct, already represented EOF cells.
use super::super::EditError;
use crate::{
    InspectError, NativeImage, TableSchema,
    format::{
        allocation::{GLOBAL_FREE_REFERENCE, MapCell, MapRole, RepresentedMap},
        reader::Reader,
    },
    table::long_value::maps::declarations,
};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn capture(
    image: &NativeImage,
    schema: &TableSchema,
    column: usize,
    new_page: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<[MapCell; 2], EditError> {
    let mut source = image.reader();
    let mut reader = Reader {
        source: &mut source,
        header: image.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let declarations = declarations(&mut reader, schema)?;
    let selected = declarations
        .entries
        .iter()
        .find(|entry| entry.number == schema.columns()[column].number())
        .ok_or(InspectError::Unsupported)?;
    if !declarations.complete || selected.references.owned() == selected.references.candidate() {
        return Err(InspectError::Unsupported.into());
    }
    let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    global.require_allocated_storage(&mut reader, &global)?;
    let global_cell = global.future_cell(&mut reader, new_page)?;
    // Global free polarity is opposite ownership: zero remains allocated at EOF.
    if global_cell.before & global_cell.mask != 0 {
        return Err(InspectError::Unsupported.into());
    }
    let owned = RepresentedMap::read(&mut reader, selected.references.owned(), MapRole::Owned)?;
    let candidate = RepresentedMap::read(
        &mut reader,
        selected.references.candidate(),
        MapRole::LongValueCandidate,
    )?;
    owned.require_allocated_storage(&mut reader, &global)?;
    candidate.require_allocated_storage(&mut reader, &global)?;
    let cells = [
        owned.future_cell(&mut reader, new_page)?,
        candidate.future_cell(&mut reader, new_page)?,
    ];
    if cells[0].offset == cells[1].offset || cells.iter().any(|cell| cell.before & cell.mask != 0) {
        return Err(InspectError::Unsupported.into());
    }
    Ok(cells)
}
