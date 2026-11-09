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
    let references = references(&mut reader, schema, column)?;
    if references.owned() == references.candidate() {
        return Err(InspectError::Unsupported.into());
    }
    let global = RepresentedMap::read(&mut reader, GLOBAL_FREE_REFERENCE, MapRole::GlobalFree)?;
    global.require_allocated_storage(&mut reader, &global)?;
    let global_cell = global.future_cell(&mut reader, new_page)?;
    // Global free polarity is opposite ownership: zero remains allocated at EOF.
    if global_cell.before & global_cell.mask != 0 {
        return Err(InspectError::Unsupported.into());
    }
    let owned = RepresentedMap::read(&mut reader, references.owned(), MapRole::Owned)?;
    let candidate = RepresentedMap::read(
        &mut reader,
        references.candidate(),
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

/// Resolve the actual selected native column pair without granting page scope.
pub(super) fn references<R: crate::NativeReadSource>(
    reader: &mut Reader<'_, R>,
    catalog: &TableSchema,
    column: usize,
) -> Result<crate::LongValueMapReferences, EditError> {
    let declared = declarations(reader, catalog)?;
    let column = catalog
        .columns()
        .get(column)
        .ok_or(InspectError::Malformed)?;
    let selected = declared
        .entries
        .iter()
        .find(|entry| entry.number == column.number())
        .ok_or(InspectError::Unsupported)?;
    if !declared.complete {
        return Err(InspectError::Unsupported.into());
    }
    Ok(selected.references)
}
