//! Independent byte envelope and recaptured appended declaration checks.
use super::super::profile;
use super::AddColumnError;
use crate::{
    InspectError, NativeImage, check_table,
    format::{bytes::u32_at, definition, layout, page::PAGE_BYTES},
    header::check,
};
use std::{io::Cursor, ops::Range};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

const FREE_BYTES: Range<usize> = 2..4;
const DEFINITION_LENGTH: Range<usize> =
    definition::LENGTH_OFFSET..definition::LENGTH_OFFSET + size_of::<u32>();
const MAX_COLUMNS: Range<usize> =
    layout::MAX_COLUMN_COUNT_OFFSET..layout::MAX_COLUMN_COUNT_OFFSET + size_of::<u16>();
const LIVE_COLUMNS: Range<usize> =
    layout::COLUMN_COUNT_OFFSET..layout::COLUMN_COUNT_OFFSET + size_of::<u16>();
const COMPARE_CHUNK_BYTES: usize = 16 * 1024;

pub(super) fn verify(
    before: &NativeImage,
    after: &[u8],
    table: &str,
    columns: &[&str],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), AddColumnError> {
    let definition = profile::USER_DEFINITION * PAGE_BYTES;
    if after.len() != before.as_bytes().len() || columns.len() < 2 {
        return Err(AddColumnError::Preservation);
    }
    let old_count = columns.len() - 1;
    let name_start = layout::COLUMN_BLOCK_OFFSET + old_count * layout::COLUMN_ENTRY_BYTES;
    let new_end = u32_at(after, definition + DEFINITION_LENGTH.start)? as usize;
    if !(name_start..=PAGE_BYTES).contains(&new_end) {
        return Err(AddColumnError::Preservation);
    }
    for (at, (original, proposed)) in before.as_bytes().iter().zip(after).enumerate() {
        if at % COMPARE_CHUNK_BYTES == 0 {
            check(budget, cancelled)?;
        }
        budget
            .charge(Resource::Steps, 1)
            .map_err(InspectError::from)?;
        let offset = at.wrapping_sub(definition);
        let allowed = FREE_BYTES.contains(&offset)
            || DEFINITION_LENGTH.contains(&offset)
            || MAX_COLUMNS.contains(&offset)
            || LIVE_COLUMNS.contains(&offset)
            || (name_start..new_end).contains(&offset);
        if original != proposed && !allowed {
            return Err(AddColumnError::Preservation);
        }
    }
    let scope = before.catalog().catalog().scope();
    let candidate = NativeImage::capture(&mut Cursor::new(after), scope, budget, cancelled)?;
    let target = check_table(&mut candidate.reader(), scope, table, budget, cancelled)?;
    if target.rows() != 0
        || !target.indexes().is_empty()
        || target.schema().columns().len() != columns.len()
    {
        return Err(AddColumnError::Preservation);
    }
    for (position, (field, name)) in target.schema().columns().iter().zip(columns).enumerate() {
        if field.name() != *name
            || field.number() != position
            || field.scalar_type() != Some(ScalarType::Int32)
            || field.flags() != (layout::FIXED_FLAG | layout::UPDATABLE_FLAG)
            || field.extended_flags() != 0
            || field.storage_length() != size_of::<i32>()
            || field.physical_field().offset != position * size_of::<i32>()
        {
            return Err(AddColumnError::Preservation);
        }
    }
    check(budget, cancelled)?;
    Ok(())
}
