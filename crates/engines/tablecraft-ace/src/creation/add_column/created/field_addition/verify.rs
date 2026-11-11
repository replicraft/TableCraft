//! Preserve original fields, native primary, properties and every unrelated byte.
use super::{FieldAdditionError as Error, FieldAdditionTarget};
use crate::creation::{add_column::common, profile};
use crate::{
    InspectError, NativeImage, TableCheck, check_table,
    format::{bytes::u32_at, definition, layout, page::PAGE_BYTES, row::COLUMN_COUNT_BYTES},
    header::check,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

pub(super) fn definition(
    before: &NativeImage,
    after: &[u8],
    original: &TableCheck,
    column: &str,
    declaration: FieldAdditionTarget,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    let schema = original.schema();
    let old_layout = schema.physical_layout();
    let definition = schema.definition_page() as usize * PAGE_BYTES;
    let old_count = schema.columns().len();
    let name_start = layout::COLUMN_BLOCK_OFFSET
        + old_layout.physical_indexes * layout::INDEX_ENTRY_BYTES
        + old_count * layout::COLUMN_ENTRY_BYTES;
    let new_end = u32_at(after, definition + definition::LENGTH_OFFSET)? as usize;
    if new_end < name_start
        || !common::preserves_variable_definition_complement(
            before.as_bytes(),
            after,
            definition,
            name_start..new_end,
            budget,
            cancelled,
        )?
    {
        return Err(Error::Preservation);
    }
    let scope = before.catalog().catalog().scope();
    let candidate = NativeImage::capture(&mut Cursor::new(after), scope, budget, cancelled)?;
    let target = check_table(
        &mut candidate.reader(),
        scope,
        schema.name(),
        budget,
        cancelled,
    )?;
    let new_schema = target.schema();
    let new_layout = new_schema.physical_layout();
    if target.rows() != 0
        || new_schema.declared_rows() != 0
        || target.indexes().len() != 1
        || target.indexes()[0].status() != original.indexes()[0].status()
        || new_schema.columns().len() != old_count + 1
        || new_layout.variable_slots
            != old_layout.variable_slots
                + usize::from(matches!(declaration, FieldAdditionTarget::ShortText { .. }))
        || new_layout.physical_indexes != old_layout.physical_indexes
        || new_layout.logical_indexes != old_layout.logical_indexes
    {
        return Err(Error::Preservation);
    }
    for (old, new) in schema.columns().iter().zip(new_schema.columns()) {
        check(budget, cancelled)?;
        if old.physical_field() != new.physical_field() {
            return Err(Error::Preservation);
        }
    }
    let appended = &new_schema.columns()[old_count];
    let (scalar, flags, extended, width, offset) = match declaration {
        FieldAdditionTarget::NumberLongZero => (
            ScalarType::Int32,
            layout::FIXED_FLAG | layout::UPDATABLE_FLAG,
            0,
            size_of::<i32>(),
            old_layout.fixed_end - COLUMN_COUNT_BYTES,
        ),
        FieldAdditionTarget::ShortText {
            max_utf16_units,
            unicode_compression,
        } => (
            ScalarType::Text,
            layout::UPDATABLE_FLAG,
            if unicode_compression {
                layout::UNICODE_COMPRESSION_FLAG
            } else {
                0
            },
            max_utf16_units
                .checked_mul(size_of::<u16>())
                .ok_or(Error::Preservation)?,
            0,
        ),
    };
    if appended.name() != column
        || appended.number() != old_count
        || appended.scalar_type() != Some(scalar)
        || appended.flags() != flags
        || appended.extended_flags() != extended
        || appended.storage_length() != width
        || appended.physical_field().offset != offset
        || appended.physical_field().variable != old_layout.variable_slots
    {
        return Err(Error::Preservation);
    }
    if matches!(declaration, FieldAdditionTarget::ShortText { .. })
        && appended.text_sort_order() != Some(profile::GENERAL_SORT)
    {
        return Err(Error::Preservation);
    }
    // Index bytes relocate after the appended field/name but remain literal.
    // This closes counters, ownership references, key direction and empty root.
    let old_end = u32_at(before.as_bytes(), definition + definition::LENGTH_OFFSET)? as usize;
    let old_tail = before
        .as_bytes()
        .get(definition + old_layout.index_start..definition + old_end)
        .ok_or(InspectError::Malformed)?;
    let new_tail = after
        .get(definition + new_layout.index_start..definition + new_end)
        .ok_or(InspectError::Malformed)?;
    if !common::equal_image(old_tail, new_tail, budget, cancelled)? {
        return Err(Error::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}
