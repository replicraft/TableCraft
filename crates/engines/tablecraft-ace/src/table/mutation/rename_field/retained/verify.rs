//! Independent name frames, exact metadata append and immutable record proof.
use super::{
    Error,
    source::{Language, Source},
};
use crate::{
    PropertyGroupKind,
    format::{
        bytes::{u16_at, u32_at},
        definition::{LENGTH_OFFSET, PAGE_HEADER_BYTES},
        page::{DATA_SIGNATURE, PAGE_BYTES},
        reader::{row_count, row_range},
        row::{reference_page, reference_slot, slot_offset},
    },
    header::check,
    properties::framing,
    read_table,
    table::mutation::{
        catalog_property::{RepeatedChange, retained_snapshot::Snapshot},
        current_schema,
        saved_field::Observed,
    },
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

const FREE_OFFSET: usize = DATA_SIGNATURE.len();
const ROW_COUNT_OFFSET: usize = 12;
const GROUP_HEADER_BYTES: usize = size_of::<u32>() + size_of::<u16>();

pub(super) fn candidate(
    source: &Source,
    after: &[u8],
    name: &str,
    change: &RepeatedChange,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    let before = source.image.as_bytes();
    let observed = &source.observed;
    if before.len() != after.len() || change.length() != before.len() {
        return Err(Error::Preservation);
    }

    let (base, length, next_length) = definition_frames(source, after, name)?;
    let reference = change.property_reference();
    let property_page = reference_page(reference);
    if property_page != reference_page(observed.property_reference.ok_or(Error::Source)?)
        || property_page != change.lval_page()
    {
        return Err(Error::Preservation);
    }
    let property_base = property_page as usize * PAGE_BYTES;
    let old_page = &before[property_base..property_base + PAGE_BYTES];
    let new_page = &after[property_base..property_base + PAGE_BYTES];
    let count = row_count(old_page)?;
    let (word, start, end) = row_range(new_page, count)?;
    if count == 0
        || row_count(new_page)? != count + 1
        || usize::from(reference_slot(reference)) != count
        || usize::from(word) != start
        || end != row_range(old_page, count - 1)?.1
        || end.checked_sub(start) != Some(change.body.len())
        || new_page[start..end] != change.body
        || usize::from(u16_at(new_page, FREE_OFFSET)?) != start - slot_offset(count + 1)
    {
        return Err(Error::Preservation);
    }

    let allowed = [
        base + FREE_OFFSET..base + FREE_OFFSET + size_of::<u16>(),
        base + LENGTH_OFFSET..base + LENGTH_OFFSET + size_of::<u32>(),
        observed.definition_name.start - size_of::<u16>()..base + length.max(next_length),
        observed.property_descriptor.start..observed.property_descriptor.end,
        property_base + FREE_OFFSET..property_base + FREE_OFFSET + size_of::<u16>(),
        property_base + ROW_COUNT_OFFSET..property_base + ROW_COUNT_OFFSET + size_of::<u16>(),
        property_base + slot_offset(count)..property_base + slot_offset(count + 1),
        property_base + start..property_base + end,
    ];
    budget.charge(Resource::Steps, (before.len() * allowed.len()) as u64)?;
    for (offset, (old, new)) in before.iter().zip(after).enumerate() {
        if offset.is_multiple_of(PAGE_BYTES) {
            check(budget, cancelled)?;
        }
        if old != new && !allowed.iter().any(|range| range.contains(&offset)) {
            return Err(Error::Preservation);
        }
    }
    metadata(source, after, name, reference, budget, cancelled)
}

fn definition_frames(
    source: &Source,
    after: &[u8],
    name: &str,
) -> Result<(usize, usize, usize), Error> {
    let before = source.image.as_bytes();
    let observed = &source.observed;
    let base = observed.schema.definition_page() as usize * PAGE_BYTES;
    let length = u32_at(before, base + LENGTH_OFFSET)? as usize;
    let difference =
        name.len() as isize * size_of::<u16>() as isize - observed.definition_name.len() as isize;
    let next_length = length
        .checked_add_signed(difference)
        .ok_or(Error::Preservation)?;
    let next_end = observed
        .definition_name
        .end
        .checked_add_signed(difference)
        .ok_or(Error::Preservation)?;
    if u32_at(after, base + LENGTH_OFFSET)? as usize != next_length
        || usize::from(u16_at(after, base + FREE_OFFSET)?)
            != PAGE_BYTES - next_length - PAGE_HEADER_BYTES
        || u16_at(after, observed.definition_name.start - size_of::<u16>())? as usize
            != name.len() * size_of::<u16>()
        || after[next_end..base + next_length]
            != before[observed.definition_name.end..base + length]
        || next_length < length
            && after[base + next_length..base + length] != before[base + next_length..base + length]
    {
        return Err(Error::Preservation);
    }
    let encoded = after
        .get(observed.definition_name.start..next_end)
        .ok_or(Error::Preservation)?;
    if !encoded
        .as_chunks::<2>()
        .0
        .iter()
        .map(|word| u16::from_le_bytes(*word))
        .eq(name.encode_utf16())
    {
        return Err(Error::Preservation);
    }

    Ok((base, length, next_length))
}

pub(super) fn retained_candidate(
    source: &Source,
    after: &[u8],
    name: &str,
    snapshot: &Snapshot<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    let before = source.image.as_bytes();
    if before.len() != after.len() {
        return Err(Error::Preservation);
    }
    let (base, length, next_length) = definition_frames(source, after, name)?;
    snapshot.verify_placement(after, budget, cancelled)?;
    let allowed = [
        base + FREE_OFFSET..base + FREE_OFFSET + size_of::<u16>(),
        base + LENGTH_OFFSET..base + LENGTH_OFFSET + size_of::<u32>(),
        source.observed.definition_name.start - size_of::<u16>()..base + length.max(next_length),
        snapshot.catalog_page_range(),
        snapshot.property_page_range(),
    ];
    budget.charge(
        Resource::Steps,
        (before.len() as u64)
            .checked_mul(allowed.len() as u64)
            .ok_or(crate::InspectError::StructureLimit)?,
    )?;
    for (offset, (old, new)) in before.iter().zip(after).enumerate() {
        if offset.is_multiple_of(PAGE_BYTES) {
            check(budget, cancelled)?;
        }
        if old != new && !allowed.iter().any(|range| range.contains(&offset)) {
            return Err(Error::Preservation);
        }
    }
    metadata(
        source,
        after,
        name,
        snapshot.property_reference(),
        budget,
        cancelled,
    )
}

fn metadata(
    source: &Source,
    after: &[u8],
    name: &str,
    reference: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    let original = &source.observed;
    let scope = source.image.catalog().catalog().scope();
    let replacement = source.family.image(after, scope, budget, cancelled)?;
    let current = source
        .family
        .observe(&replacement, original.schema.name(), budget, cancelled)
        .map_err(super::super::current_source::source_error)?;
    let current_schema::UserStorage::Single { reference: actual } = current.user_storage else {
        return Err(Error::Preservation);
    };
    if reference != actual
        || current.schema.columns().len() != original.schema.columns().len()
        || current.schema.object_id() != original.schema.object_id()
        || current.schema.definition_page() != original.schema.definition_page()
        || current.schema.declared_rows() != original.schema.declared_rows()
    {
        return Err(Error::Preservation);
    }
    for (old, new) in original
        .schema
        .columns()
        .iter()
        .zip(current.schema.columns())
    {
        check(budget, cancelled)?;
        let expected_name = if old.number() == original.identity.number {
            name
        } else {
            old.name()
        };
        if new.name() != expected_name || !old.physical_field().same_storage(new.physical_field()) {
            return Err(Error::Preservation);
        }
    }
    let selected = match source.family {
        current_schema::Family::Current => Observed::capture_current_rename(
            &replacement,
            original.schema.name(),
            name,
            budget,
            cancelled,
        )?,
        current_schema::Family::Retained => match source.language {
            Language::LongText => Observed::capture_retained_schema_rename(
                &replacement,
                original.schema.name(),
                name,
                budget,
                cancelled,
            )?,
            Language::Typed => Observed::capture_retained_typed_rename(
                &replacement,
                original.schema.name(),
                name,
                budget,
                cancelled,
            )?,
        },
    };
    if selected.identity.native_table != original.identity.native_table
        || selected.identity.definition != original.identity.definition
        || selected.identity.number != original.identity.number
        || selected.identity.column_id != original.identity.column_id
        || selected.property_reference != Some(reference)
    {
        return Err(Error::Preservation);
    }
    framing::current_schema_names(&current.properties, budget, cancelled)?;
    property_frames(original, &current.properties, name, budget, cancelled)?;

    let old_rows = read_table(
        &mut source.image.reader(),
        scope,
        original.schema.name(),
        budget,
        cancelled,
    )?;
    let new_rows = read_table(
        &mut Cursor::new(after),
        scope,
        original.schema.name(),
        budget,
        cancelled,
    )?;
    if old_rows.rows() != new_rows.rows() {
        return Err(Error::Preservation);
    }
    Ok(())
}

fn property_frames(
    original: &crate::table::mutation::saved_field::Observed,
    replacement: &crate::TableProperties,
    name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    if replacement.groups().len() != original.properties.groups().len() {
        return Err(Error::Preservation);
    }
    let mut group = None;
    framing::current_schema_group_blocks(
        &original.properties,
        budget,
        cancelled,
        |index, span| {
            if index == original.group {
                group = Some(span);
            }
            Ok(())
        },
    )?;
    let group = group.ok_or(Error::Preservation)?;
    let old = original.properties.raw_bytes();
    let new = replacement.raw_bytes();
    let old_start = original.property_name.start - original.property_body.start;
    let old_end = original.property_name.end - original.property_body.start;
    let delta = name.len() as isize * size_of::<u16>() as isize - (old_end - old_start) as isize;
    let new_end = old_end
        .checked_add_signed(delta)
        .ok_or(Error::Preservation)?;
    let group_length = group
        .len()
        .checked_add_signed(delta)
        .ok_or(Error::Preservation)?;
    let name_frame = group.start + GROUP_HEADER_BYTES;
    let name_frame_length = (u32_at(old, name_frame)? as usize)
        .checked_add_signed(delta)
        .ok_or(Error::Preservation)?;
    if old.len().checked_add_signed(delta) != Some(new.len())
        || new[new_end..] != old[old_end..]
        || u32_at(new, group.start)? as usize != group_length
        || u32_at(new, name_frame)? as usize != name_frame_length
        || usize::from(u16_at(new, old_start - size_of::<u16>())?) != name.len() * size_of::<u16>()
    {
        return Err(Error::Preservation);
    }
    let mutable = [
        group.start..group.start + size_of::<u32>(),
        name_frame..name_frame + size_of::<u32>(),
        old_start - size_of::<u16>()..old_start,
    ];
    budget.charge(Resource::Steps, old_start as u64)?;
    for at in 0..old_start {
        check(budget, cancelled)?;
        if !mutable.iter().any(|range| range.contains(&at)) && old[at] != new[at] {
            return Err(Error::Preservation);
        }
    }
    if !new[old_start..new_end]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|unit| u16::from_le_bytes(*unit))
        .eq(name.encode_utf16())
        || replacement.groups()[original.group].kind() != PropertyGroupKind::Column
        || replacement.groups()[original.group].name() != name
    {
        return Err(Error::Preservation);
    }
    Ok(())
}
