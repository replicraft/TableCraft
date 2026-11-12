//! Fresh source reconstruction and byte relations, independent of staged buffers.
use super::{definition, retirement, source};
use crate::{
    EditError, InspectError, NativeImage,
    format::{
        bytes::{u16_at, u32_at},
        definition::{LENGTH_OFFSET, PAGE_HEADER_BYTES},
        layout::{INDEX_COUNT_OFFSET, LOGICAL_INDEX_COUNT_OFFSET},
        page::PAGE_BYTES,
    },
    header::check,
    table::{
        indexes::definition::{DATA_NUMBER_OFFSET, LOGICAL_BYTES},
        mutation::normal_ordinary,
        table_append::Inventory,
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_OFFSET: usize = 2;

pub(super) fn prove(
    before: &NativeImage,
    after: &NativeImage,
    table: &str,
    requested: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    prove_policy(
        before,
        after,
        table,
        requested,
        source::Policy::Ordinary,
        budget,
        cancelled,
    )
}

pub(super) fn prove_policy(
    before: &NativeImage,
    after: &NativeImage,
    table: &str,
    requested: &str,
    policy: source::Policy,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let original = source::capture_policy(before, table, requested, policy, budget, cancelled)?;
    Inventory::capture(after, budget, cancelled)?.require_source(after)?;
    let current = normal_ordinary::capture_retained_schema(after, table, budget, cancelled)?;
    let old_schema = original.content.table.schema();
    let new_schema = current.table.schema();
    let removed_physical = u32::from(original.remove_physical);
    if old_schema.object_id() != new_schema.object_id()
        || old_schema.definition_page() != new_schema.definition_page()
        || old_schema.declared_rows() != new_schema.declared_rows()
        || current.records.len() != original.content.records.len()
        || current.indexes.indexes().len() + 1 != original.content.indexes.indexes().len()
        || !current.dependencies.is_empty()
    {
        return Err(EditError::Preservation);
    }
    let observed = super::super::index_creation::definition::DefinitionLayout::read(
        after, new_schema, budget, cancelled,
    )?;
    if observed.physical_count + removed_physical != original.layout.physical_count
        || observed.logical_count + 1 != original.layout.logical_count
    {
        return Err(EditError::Preservation);
    }
    let mut remaining = current.indexes.indexes().iter();
    for (position, old) in original.content.indexes.indexes().iter().enumerate() {
        check(budget, cancelled)?;
        if position == original.position {
            continue;
        }
        let new = remaining.next().ok_or(EditError::Preservation)?;
        let physical = old.data_number()
            - u32::from(
                original.remove_physical && old.data_number() > original.selected().data_number(),
            );
        budget
            .charge(
                Resource::Steps,
                old.name().len().max(new.name().len()) as u64,
            )
            .map_err(InspectError::from)?;
        if old.name() != new.name()
            || old.number() != new.number()
            || physical != new.data_number()
            || old.root_page() != new.root_page()
            || old.usage_map() != new.usage_map()
            || old.native_type() != new.native_type()
            || old.flags() != new.flags()
            || old.columns() != new.columns()
            || old.foreign_reference() != new.foreign_reference()
        {
            return Err(EditError::Preservation);
        }
    }
    let start = original.layout.definition as usize * PAGE_BYTES;
    let old = &before.as_bytes()[start..start + PAGE_BYTES];
    let new = &after.as_bytes()[start..start + PAGE_BYTES];
    let removed = definition::removed(before, &original, budget, cancelled)?;
    let length = original
        .layout
        .length
        .checked_sub(removed.iter().map(std::ops::Range::len).sum::<usize>())
        .ok_or(EditError::Preservation)?;
    if observed.length != length
        || u32_at(new, LENGTH_OFFSET)? as usize != length
        || usize::from(u16_at(new, FREE_OFFSET)?)
            != PAGE_BYTES.saturating_sub(length + PAGE_HEADER_BYTES)
    {
        return Err(EditError::Preservation);
    }
    // Verify the monotone original-byte relation rather than rerunning compose.
    // Only the physically compacted reference words and four header fields differ.
    let fields = [
        FREE_OFFSET..FREE_OFFSET + 2,
        LENGTH_OFFSET..LENGTH_OFFSET + 4,
        INDEX_COUNT_OFFSET..INDEX_COUNT_OFFSET + 4,
        LOGICAL_INDEX_COUNT_OFFSET..LOGICAL_INDEX_COUNT_OFFSET + 4,
    ];
    let mut at = 0;
    for (old_at, value) in old.iter().enumerate().take(original.layout.length) {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, (removed.len() + fields.len() + 1) as u64)
            .map_err(InspectError::from)?;
        if removed.iter().any(|span| span.contains(&old_at)) {
            continue;
        }
        let reference = if (observed.logical.start..observed.logical.end).contains(&at) {
            let within = (at - observed.logical.start) % LOGICAL_BYTES;
            (DATA_NUMBER_OFFSET..DATA_NUMBER_OFFSET + size_of::<u32>()).contains(&within)
        } else {
            false
        };
        if !fields.iter().any(|field| field.contains(&at))
            && !reference
            && new.get(at) != Some(value)
        {
            return Err(EditError::Preservation);
        }
        at += 1;
    }
    budget
        .charge(Resource::Steps, (PAGE_BYTES - length) as u64)
        .map_err(InspectError::from)?;
    if at != length
        || new[length..original.layout.length]
            .iter()
            .any(|value| *value != 0)
        || old[original.layout.length..] != new[original.layout.length..]
    {
        return Err(EditError::Preservation);
    }
    // Parsed data_number equality above checks each exempted reference word;
    // native identities and the actual count/free/length laws check header fields.
    let edits = retirement::cells(before, &original.retired, budget, cancelled)?;
    if before.as_bytes().len() != after.as_bytes().len() {
        return Err(EditError::Preservation);
    }
    for (page, (old, new)) in before
        .as_bytes()
        .as_chunks::<PAGE_BYTES>()
        .0
        .iter()
        .zip(after.as_bytes().as_chunks::<PAGE_BYTES>().0)
        .enumerate()
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, PAGE_BYTES as u64)
            .map_err(InspectError::from)?;
        if page == original.layout.definition as usize {
            continue;
        }
        if original.retired.binary_search(&(page as u32)).is_ok() {
            if new.iter().any(|byte| *byte != 0) {
                return Err(EditError::Preservation);
            }
            continue;
        }
        if old == new {
            continue;
        }
        for (within, (old, new)) in old.iter().zip(new).enumerate() {
            check(budget, cancelled)?;
            if old == new {
                continue;
            }
            budget
                .charge(Resource::Steps, edits.len() as u64)
                .map_err(InspectError::from)?;
            let offset = page * PAGE_BYTES + within;
            let edit = edits
                .iter()
                .find(|edit| edit.offset == offset)
                .ok_or(EditError::Preservation)?;
            if *old != edit.before || *new != (edit.before | edit.set) || edit.clear != 0 {
                return Err(EditError::Preservation);
            }
        }
    }
    for edit in edits {
        check(budget, cancelled)?;
        if after.as_bytes().get(edit.offset) != Some(&(edit.before | edit.set)) {
            return Err(EditError::Preservation);
        }
    }
    Ok(())
}
