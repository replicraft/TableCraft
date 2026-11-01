//! Independent nullable-I32 component extents, ROW and NODE suffix grammar.
use crate::{
    EditError, InspectError, NativeIndex, WriteBlocker,
    header::check,
    table::indexes::key::{ASC_NULL, ASC_VALUE, DESC_NULL, DESC_VALUE},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) struct Facts {
    /// Complete components plus the ROW suffix, excluding NODE child bytes.
    pub key_end: usize,
    pub all_null: bool,
    pub any_null: bool,
}

fn consume(entry: &[u8], cursor: &mut usize, width: usize) -> Result<(), EditError> {
    let end = cursor.checked_add(width).ok_or(EditError::Preservation)?;
    entry.get(*cursor..end).ok_or(EditError::Preservation)?;
    *cursor = end;
    Ok(())
}

pub(super) fn inspect(
    entry: &[u8],
    index: &NativeIndex,
    leaf: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Facts, EditError> {
    if index.columns().is_empty() {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    }
    let mut cursor = 0_usize;
    let mut all_null = true;
    let mut any_null = false;
    for column in index.columns() {
        check(budget, cancelled)?;
        let marker = *entry.get(cursor).ok_or(EditError::Preservation)?;
        let null = if column.descending() {
            DESC_NULL
        } else {
            ASC_NULL
        };
        let present = if column.descending() {
            DESC_VALUE
        } else {
            ASC_VALUE
        };
        if marker == null {
            consume(entry, &mut cursor, size_of::<u8>())?;
            any_null = true;
        } else if marker == present {
            consume(entry, &mut cursor, size_of::<u8>() + size_of::<i32>())?;
            all_null = false;
        } else {
            return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
        }
    }
    consume(entry, &mut cursor, size_of::<u32>())?;
    let key_end = cursor;
    if !leaf {
        consume(entry, &mut cursor, size_of::<u32>())?;
    }
    if cursor != entry.len() {
        return Err(EditError::Preservation);
    }
    budget
        .charge(Resource::Steps, cursor as u64)
        .map_err(InspectError::from)?;
    Ok(Facts {
        key_end,
        all_null,
        any_null,
    })
}
