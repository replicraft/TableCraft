//! Independent nullable-I32 component extents, ROW and NODE suffix grammar.
use crate::{
    EditError, InspectError, NativeIndex, WriteBlocker,
    header::check,
    table::indexes::key::{ASC_NULL, ASC_VALUE, DESC_NULL, DESC_VALUE},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const GUID_SEGMENT_BYTES: usize = 8;
const GUID_CONTINUED_TAG: u8 = 9;
const GUID_FIRST_TAG: usize = 1 + GUID_SEGMENT_BYTES;
const GUID_LAST_TAG: usize = GUID_FIRST_TAG + 1 + GUID_SEGMENT_BYTES;
const GUID_COMPONENT_BYTES: usize = GUID_LAST_TAG + 1;

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
    inspect_width(entry, index, leaf, size_of::<i32>(), budget, cancelled)
}

pub(super) fn inspect_width(
    entry: &[u8],
    index: &NativeIndex,
    leaf: bool,
    value_bytes: usize,
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
            consume(entry, &mut cursor, size_of::<u8>() + value_bytes)?;
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

/// Two fixed eight-byte segments; ROW is never complemented or decoded as data.
pub(super) fn inspect_guid(
    entry: &[u8],
    index: &NativeIndex,
    leaf: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Facts, EditError> {
    check(budget, cancelled)?;
    let [key] = index.columns() else {
        return Err(EditError::Unsupported(WriteBlocker::IndexedColumn));
    };
    let marker = *entry.first().ok_or(EditError::Preservation)?;
    let (null, present, final_tag) = if key.descending() {
        (0xff, 0x80, 0xf7)
    } else {
        (0, 0x7f, 8)
    };
    let component = if marker == null {
        1
    } else if marker == present
        && entry.get(GUID_FIRST_TAG) == Some(&GUID_CONTINUED_TAG)
        && entry.get(GUID_LAST_TAG) == Some(&final_tag)
    {
        GUID_COMPONENT_BYTES
    } else {
        return Err(EditError::Preservation);
    };
    let key_end = component + size_of::<u32>();
    if entry.len() != key_end + if leaf { 0 } else { size_of::<u32>() } {
        return Err(EditError::Preservation);
    }
    budget
        .charge(Resource::Steps, entry.len() as u64)
        .map_err(InspectError::from)?;
    Ok(Facts {
        key_end,
        all_null: marker == null,
        any_null: marker == null,
    })
}
