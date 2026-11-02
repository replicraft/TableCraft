//! Original-cell mask composition shared by actual INDEX and LVAL retirement.
use crate::{
    EditError, InspectError,
    format::{allocation::MapCell, reader::Reader},
};

pub(in crate::table) struct ByteEdit {
    pub offset: usize,
    pub before: u8,
    pub set: u8,
    pub clear: u8,
}

pub(in crate::table) fn merge(
    edits: &mut Vec<ByteEdit>,
    cell: MapCell,
    set: bool,
    reader: &mut Reader<'_, std::io::Cursor<&[u8]>>,
) -> Result<(), EditError> {
    reader.work(edits.len())?;
    let current = cell.before & cell.mask != 0;
    if current == set {
        return Err(InspectError::Malformed.into());
    }
    if let Some(edit) = edits.iter_mut().find(|edit| edit.offset == cell.offset) {
        if edit.before != cell.before || (edit.set | edit.clear) & cell.mask != 0 {
            return Err(EditError::Preservation);
        }
        if set {
            edit.set |= cell.mask;
        } else {
            edit.clear |= cell.mask;
        }
    } else {
        reader
            .budget
            .charge_allocation(1, size_of::<ByteEdit>() as u64)
            .map_err(InspectError::from)?;
        edits.try_reserve(1).map_err(|_| InspectError::Allocation)?;
        edits.push(ByteEdit {
            offset: cell.offset,
            before: cell.before,
            set: if set { cell.mask } else { 0 },
            clear: if set { 0 } else { cell.mask },
        });
    }
    Ok(())
}
