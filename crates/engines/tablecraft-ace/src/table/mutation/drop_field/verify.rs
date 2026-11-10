//! Independent decoded survivors and exact byte complement after fitted append.
use super::{
    FieldDropError as Error,
    source::{Evidence, Purpose},
};
use crate::{
    format::{
        bytes::{u16_at, u32_at},
        definition::LENGTH_OFFSET,
        layout::COLUMN_COUNT_OFFSET,
        page::PAGE_BYTES,
        reader::{row_count, row_range},
        row::{OFFSET_MASK, ROW_COUNT_OFFSET, SLOT_BYTES, reference, slot_offset},
    },
    header::check,
    inspect_catalog, inspect_database_properties, inspect_table_indexes, inspect_table_properties,
    inspect_table_schema,
    properties::framing,
    table::mutation::catalog_property::{RepeatedChange, ledger},
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

const CATALOG_PAGE: usize = 4;
const PROPERTY_PAGE: usize = 10;
const FREE_BYTES_OFFSET: usize = 2;
const DEFINITION_FOOTER_BYTES: usize = 8;

pub(super) fn candidate(
    before: &crate::NativeImage,
    evidence: &Evidence,
    after: &[u8],
    change: &RepeatedChange,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    check(budget, cancelled)?;
    let observed = &evidence.observed;
    if before.as_bytes().len() != after.len()
        || change.length() != after.len()
        || change.catalog_page != CATALOG_PAGE as u32
        || change.lval_page() != PROPERTY_PAGE as u32
    {
        return Err(Error::Preservation);
    }
    let definition_start = observed.schema.definition_page() as usize * PAGE_BYTES;
    definition_bytes(before, evidence, after, budget, cancelled)?;
    let property_start = PROPERTY_PAGE * PAGE_BYTES;
    let old = &before.as_bytes()[property_start..property_start + PAGE_BYTES];
    let new = &after[property_start..property_start + PAGE_BYTES];
    let count = row_count(old)?;
    let slot = u8::try_from(count).map_err(|_| Error::Preservation)?;
    let expected_reference = reference(PROPERTY_PAGE as u32, slot);
    let (row_word, start, end) = row_range(new, count)?;
    let (previous_word, old_start, _) =
        row_range(old, count.checked_sub(1).ok_or(Error::Preservation)?)?;
    let expected_free = start
        .checked_sub(slot_offset(count + 1))
        .ok_or(Error::Preservation)?;
    budget.charge(Resource::Steps, (change.body.len() * 3) as u64)?;
    if row_count(new)? != count + 1
        || row_word & !OFFSET_MASK != 0
        || previous_word & !OFFSET_MASK != 0
        || end != old_start
        || end - start != change.body.len()
        || new[start..end] != change.body
        || old[start..end].iter().any(|byte| *byte != 0)
        || u16_at(new, FREE_BYTES_OFFSET)? as usize != expected_free
        || change.property_reference() != expected_reference
    {
        return Err(Error::Preservation);
    }
    let definition_span = definition_start..definition_start + PAGE_BYTES;
    let body_span = property_start + start..property_start + end;
    let directory_span =
        property_start + slot_offset(count)..property_start + slot_offset(count) + SLOT_BYTES;
    let count_span =
        property_start + ROW_COUNT_OFFSET..property_start + ROW_COUNT_OFFSET + size_of::<u16>();
    let free_span =
        property_start + FREE_BYTES_OFFSET..property_start + FREE_BYTES_OFFSET + size_of::<u16>();
    budget.charge(Resource::Steps, (before.as_bytes().len() * 6) as u64)?;
    for (offset, (left, right)) in before.as_bytes().iter().zip(after).enumerate() {
        if offset.is_multiple_of(PAGE_BYTES) {
            check(budget, cancelled)?;
        }
        if left != right
            && ![
                &definition_span,
                &evidence.descriptor,
                &body_span,
                &directory_span,
                &count_span,
                &free_span,
            ]
            .iter()
            .any(|span| span.contains(&offset))
        {
            return Err(Error::Preservation);
        }
    }
    let scope = before.catalog().catalog().scope();
    let native = inspect_catalog(&mut Cursor::new(after), scope, budget, cancelled)?;
    if native.header() != before.catalog().header()
        || native.origins().len() != before.catalog().origins().len()
    {
        return Err(Error::Preservation);
    }
    for (left, right) in before.catalog().origins().iter().zip(native.origins()) {
        check(budget, cancelled)?;
        if left.object_id() != right.object_id()
            || left.native_id() != right.native_id()
            || left.parent_id() != right.parent_id()
            || left.type_code() != right.type_code()
            || left.flags() != right.flags()
            || left.page() != right.page()
            || left.slot() != right.slot()
        {
            return Err(Error::Preservation);
        }
    }
    let schema = inspect_table_schema(
        &mut Cursor::new(after),
        scope,
        observed.schema.name(),
        budget,
        cancelled,
    )?;
    let original_layout = observed.schema.physical_layout();
    let new_layout = schema.physical_layout();
    if schema.object_id() != observed.schema.object_id()
        || schema.name() != observed.schema.name()
        || schema.definition_page() != observed.schema.definition_page()
        || schema.columns().len() + 1 != observed.schema.columns().len()
        || new_layout.max_columns != original_layout.max_columns
        || new_layout.variable_slots != original_layout.variable_slots
        || new_layout.records != original_layout.records
        || new_layout.usage != original_layout.usage
        || new_layout.physical_indexes != original_layout.physical_indexes
        || new_layout.logical_indexes != original_layout.logical_indexes
    {
        return Err(Error::Preservation);
    }
    for (original, current) in observed
        .schema
        .columns()
        .iter()
        .filter(|column| column.number() != observed.identity.column_number())
        .zip(schema.columns())
    {
        check(budget, cancelled)?;
        budget.charge(
            Resource::Steps,
            (original.name().len() + current.name().len()) as u64,
        )?;
        if original.name() != current.name()
            || !original
                .physical_field()
                .same_storage(current.physical_field())
        {
            return Err(Error::Preservation);
        }
    }
    // Successful native index reconstruction must still resolve the retained
    // original field numbers; full TDEF/index bytes are separately protected.
    inspect_table_indexes(
        &mut Cursor::new(after),
        scope,
        schema.name(),
        budget,
        cancelled,
    )?;
    let properties = inspect_table_properties(
        &mut Cursor::new(after),
        scope,
        schema.name(),
        budget,
        cancelled,
    )?;
    match evidence.purpose {
        Purpose::SettledEmpty => framing::validate(&properties, budget, cancelled)?,
        Purpose::CurrentTail => {
            // The complete original dictionary and every original group have
            // strict frame/entry coverage. The exact group-omission proof below
            // carries that coverage to AFTER, including dictionary names whose
            // last use was removed and a dictionary with no surviving groups.
            // Those retained bytes are not permission for an orphan live group.
            framing::validate(&observed.properties, budget, cancelled)?;
        }
    }
    if properties.object_id() != observed.properties.object_id()
        || properties.name() != observed.properties.name()
        || properties.raw_bytes() != change.body
        || properties.groups().len() + 1 != observed.properties.groups().len()
    {
        return Err(Error::Preservation);
    }
    let mut removed = None;
    framing::ordered_group_blocks(&observed.properties, budget, cancelled, |group, span| {
        if group == observed.group && removed.replace(span).is_some() {
            return Err(crate::InspectError::Malformed);
        }
        Ok(())
    })?;
    let removed = removed.ok_or(Error::Preservation)?;
    let raw = observed.properties.raw_bytes();
    budget.charge(Resource::Steps, raw.len() as u64)?;
    if change.body.len() != raw.len() - removed.len()
        || change.body[..removed.start] != raw[..removed.start]
        || change.body[removed.start..] != raw[removed.end..]
    {
        return Err(Error::Preservation);
    }
    let old_database = inspect_database_properties(&mut before.reader(), scope, budget, cancelled)?
        .ok_or(Error::Preservation)?;
    let database = inspect_database_properties(&mut Cursor::new(after), scope, budget, cancelled)?
        .ok_or(Error::Preservation)?;
    budget.charge(
        Resource::Steps,
        (old_database.raw_bytes().len() + database.raw_bytes().len()) as u64,
    )?;
    if database.object_id() != old_database.object_id()
        || database.raw_bytes() != old_database.raw_bytes()
    {
        return Err(Error::Preservation);
    }
    let old_ledger = ledger::capture(before.as_bytes(), before.catalog(), budget, cancelled)?;
    let (database_reference, database_length) = old_ledger
        .single_reference(old_database.object_id(), budget, cancelled)?
        .ok_or(Error::Preservation)?;
    let live = ledger::capture(after, &native, budget, cancelled)?;
    if !live.two_singles(
        [
            (schema.object_id(), expected_reference, change.body.len()),
            (database.object_id(), database_reference, database_length),
        ],
        budget,
        cancelled,
    )? {
        return Err(Error::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}

/// Verify the native transform directly from original parser-owned ranges, not
/// from a producer-generated replacement page. Every retained source byte is
/// located independently; only the three stored header facts may change.
fn definition_bytes(
    before: &crate::NativeImage,
    evidence: &Evidence,
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    check(budget, cancelled)?;
    let observed = &evidence.observed;
    let base = observed.schema.definition_page() as usize * PAGE_BYTES;
    let old = &before.as_bytes()[base..base + PAGE_BYTES];
    let new = after
        .get(base..base + PAGE_BYTES)
        .ok_or(Error::Preservation)?;
    let entry_start = observed.definition_entry.start - base;
    let entry_end = observed.definition_entry.end - base;
    let name_start = observed.definition_name.start - base - size_of::<u16>();
    let name_end = observed.definition_name.end - base;
    let old_length = u32_at(old, LENGTH_OFFSET)? as usize;
    let new_length = u32_at(new, LENGTH_OFFSET)? as usize;
    let removed = entry_end - entry_start + name_end - name_start;
    let expected_length = old_length.checked_sub(removed).ok_or(Error::Preservation)?;
    let expected_free = PAGE_BYTES
        .checked_sub(expected_length + DEFINITION_FOOTER_BYTES)
        .ok_or(Error::Preservation)?;
    if new_length != expected_length
        || u16_at(new, COLUMN_COUNT_OFFSET)?.checked_add(1)
            != Some(u16_at(old, COLUMN_COUNT_OFFSET)?)
        || u16_at(new, FREE_BYTES_OFFSET)? as usize != expected_free
    {
        return Err(Error::Preservation);
    }
    let headers = [
        LENGTH_OFFSET..LENGTH_OFFSET + size_of::<u32>(),
        COLUMN_COUNT_OFFSET..COLUMN_COUNT_OFFSET + size_of::<u16>(),
        FREE_BYTES_OFFSET..FREE_BYTES_OFFSET + size_of::<u16>(),
    ];
    budget.charge(Resource::Steps, (PAGE_BYTES * 2) as u64)?;
    for offset in 0..entry_start {
        if old[offset] != new[offset] && !headers.iter().any(|span| span.contains(&offset)) {
            return Err(Error::Preservation);
        }
    }
    let middle_end = entry_start + name_start - entry_end;
    if new[entry_start..middle_end] != old[entry_end..name_start]
        || new[middle_end..new_length] != old[name_end..old_length]
        || new[new_length..].iter().any(|byte| *byte != 0)
    {
        return Err(Error::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}
