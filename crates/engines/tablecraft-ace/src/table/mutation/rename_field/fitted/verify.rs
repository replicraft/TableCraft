//! Relocation-aware complement and semantic proof, independent of the repacker.
use super::super::{FieldRenameError as Error, source::Qualified, verify};
use super::{
    build::shifted,
    layout::{FREE_OFFSET, Layout},
};
use crate::{
    format::{
        bytes::{u16_at, u32_at},
        definition::LENGTH_OFFSET,
        page::PAGE_BYTES,
        reader::{row_count, row_range},
        row::slot_offset,
    },
    header::check,
    inspect_database_properties,
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

const GROUP_HEADER_BYTES: usize = size_of::<u32>() + size_of::<u16>();

pub(super) fn candidate(
    source: &Qualified,
    layout: &Layout,
    after: &[u8],
    name: &str,
    delta: isize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    let before = source.image();
    let observed = &source.observed;
    if after.len() != before.as_bytes().len() {
        return Err(Error::Preservation);
    }

    let definition_length = shifted(layout.definition_length, delta)?;
    let user_start = shifted(layout.property_start, -delta)?;
    let database_start = shifted(layout.database_start, -delta)?;
    let property_bytes = shifted(observed.property_body.len(), delta)?;
    let definition = layout.definition_base;
    let property = layout.property_base;
    let descriptor = observed.property_descriptor.start;
    let permitted = [
        definition + FREE_OFFSET..definition + FREE_OFFSET + size_of::<u16>(),
        definition + LENGTH_OFFSET..definition + LENGTH_OFFSET + size_of::<u32>(),
        observed.definition_name.start - size_of::<u16>()
            ..definition + layout.definition_length.max(definition_length),
        property + FREE_OFFSET..property + FREE_OFFSET + size_of::<u16>(),
        property + slot_offset(layout.property_slot)
            ..property + slot_offset(layout.property_slot + 2),
        property + layout.database_start.min(database_start)..property + layout.property_end,
        descriptor..descriptor + size_of::<u32>(),
    ];
    budget.charge(Resource::Steps, (after.len() * permitted.len()) as u64)?;
    for (offset, (old, new)) in before.as_bytes().iter().zip(after).enumerate() {
        if offset.is_multiple_of(PAGE_BYTES) {
            check(budget, cancelled)?;
        }
        if old != new && !permitted.iter().any(|range| range.contains(&offset)) {
            return Err(Error::Preservation);
        }
    }

    let original_page = &before.as_bytes()[property..property + PAGE_BYTES];
    let page = &after[property..property + PAGE_BYTES];
    let (word, start, end) = row_range(page, layout.property_slot)?;
    let (database_word, database_start_actual, database_end) =
        row_range(page, layout.property_slot + 1)?;
    if row_count(page)? != row_count(original_page)?
        || usize::from(word) != user_start
        || usize::from(database_word) != database_start
        || start != user_start
        || end != layout.property_end
        || database_start_actual != database_start
        || database_end != user_start
        || usize::from(u16_at(page, FREE_OFFSET)?) != shifted(layout.free, -delta)?
        || original_page[layout.database_start..layout.property_start]
            != page[database_start..user_start]
        || u32_at(after, definition + LENGTH_OFFSET)? as usize != definition_length
        || usize::from(u16_at(after, definition + FREE_OFFSET)?)
            != shifted(layout.definition_free, -delta)?
    {
        return Err(Error::Preservation);
    }

    // Descriptor record ownership and canonical Single framing were proved on
    // the original. Only its length changes; reference/reserved bytes stay exact.
    let original_tagged = u32_at(before.as_bytes(), descriptor)? as usize;
    if u32_at(after, descriptor)? as usize != shifted(original_tagged, delta)? {
        return Err(Error::Preservation);
    }
    let definition_name_end = shifted(observed.definition_name.end, delta)?;
    if before.as_bytes()[observed.definition_name.end..definition + layout.definition_length]
        != after[definition_name_end..definition + definition_length]
    {
        return Err(Error::Preservation);
    }

    let original_body = observed.properties.raw_bytes();
    let body = &page[user_start..layout.property_end];
    let name_start = observed.property_name.start - observed.property_body.start;
    let name_end = observed.property_name.end - observed.property_body.start;
    let name_block = layout.group.start + GROUP_HEADER_BYTES;
    if body.len() != property_bytes
        || original_body[..layout.group.start] != body[..layout.group.start]
        || original_body[layout.group.start + size_of::<u32>()..name_block]
            != body[layout.group.start + size_of::<u32>()..name_block]
        || original_body[name_end..] != body[shifted(name_end, delta)?..]
        || u32_at(body, layout.group.start)? as usize != shifted(layout.group.len(), delta)?
        || u32_at(body, name_block)? as usize
            != shifted(u32_at(original_body, name_block)? as usize, delta)?
        || usize::from(u16_at(body, name_start - size_of::<u16>())?)
            != name.len() * size_of::<u16>()
    {
        return Err(Error::Preservation);
    }

    // Shrinking exposes historical bytes as free space. Preserve them exactly
    // instead of treating the newly free ranges as permission to erase them.
    if delta < 0
        && (before.as_bytes()
            [definition + definition_length..definition + layout.definition_length]
            != after[definition + definition_length..definition + layout.definition_length]
            || original_page[layout.database_start..database_start]
                != page[layout.database_start..database_start])
    {
        return Err(Error::Preservation);
    }
    let scope = before.catalog().catalog().scope();
    let old_options = inspect_database_properties(&mut before.reader(), scope, budget, cancelled)?
        .ok_or(Error::Preservation)?;
    let new_options =
        inspect_database_properties(&mut Cursor::new(after), scope, budget, cancelled)?
            .ok_or(Error::Preservation)?;
    budget.charge(
        Resource::Steps,
        (old_options.raw_bytes().len() + new_options.raw_bytes().len()) as u64,
    )?;
    if old_options.object_id() != new_options.object_id()
        || old_options.raw_bytes() != new_options.raw_bytes()
    {
        return Err(Error::Preservation);
    }

    verify::metadata(
        source,
        after,
        name,
        property_bytes,
        delta,
        budget,
        cancelled,
    )
}
