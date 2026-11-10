//! Independent original-byte envelope and decoded appended native role checks.
use crate::creation::catalog_page::Page;
pub(in crate::creation::add_table) mod catalog;
pub(in crate::creation::add_table) mod definition;
pub(in crate::creation::add_table) mod empty_pages;
pub(in crate::creation::add_table) mod properties;
use super::super::FirstTableDefinition;
use super::{
    AddTableError,
    source::{self, Source},
};
use crate::{
    CreateColumn, IndexCheckStatus, NativeImage, NativeSortOrder, PayloadPresence, check_table,
    format::{layout::RECORD_COUNT_OFFSET, page::PAGE_BYTES, row::reference},
    header::check,
};
use std::{io::Cursor, ops::Range};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

const CATALOG_DEFINITION: usize = 2;
const HEADER_BYTES: usize = 14;
const SLOT_BYTES: usize = size_of::<u16>();

fn permitted(page: usize, offset: usize, source: &Source, maps: &Page, catalog: &Page) -> bool {
    if page == CATALOG_DEFINITION {
        return (RECORD_COUNT_OFFSET..RECORD_COUNT_OFFSET + size_of::<u32>()).contains(&offset);
    }
    let (old, new) = if page == source::MAP_PAGE {
        (&source.maps, maps)
    } else if page == source::CATALOG_PAGE {
        (&source.catalog, catalog)
    } else {
        return false;
    };
    let new_slots: Range<usize> =
        HEADER_BYTES + old.count * SLOT_BYTES..HEADER_BYTES + new.count * SLOT_BYTES;
    (2..4).contains(&offset)
        || (12..14).contains(&offset)
        || new_slots.contains(&offset)
        || (new.start..old.start).contains(&offset)
}

pub(super) fn verify(
    before: &NativeImage,
    after: &[u8],
    definition: &FirstTableDefinition,
    source: &Source,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), AddTableError> {
    let role = source.geometry.added;
    if after.len() != source.geometry.after_pages * PAGE_BYTES {
        return Err(AddTableError::Preservation);
    }
    empty_pages::verify(after, role, budget, cancelled)?;
    let maps = Page::capture(source::page(after, source::MAP_PAGE), 0, budget, cancelled)?;
    let catalog = Page::capture(
        source::page(after, source::CATALOG_PAGE),
        CATALOG_DEFINITION as u32,
        budget,
        cancelled,
    )?;
    if maps.count != source.maps.count + role.pages() || catalog.count != source.catalog.count + 1 {
        return Err(AddTableError::Preservation);
    }
    if source::page(after, CATALOG_DEFINITION)
        [RECORD_COUNT_OFFSET..RECORD_COUNT_OFFSET + size_of::<u32>()]
        != ((source.catalog.count + 1) as u32).to_le_bytes()
    {
        return Err(AddTableError::Preservation);
    }
    for (range, member) in maps.records[source.maps.count..].iter().zip(role.members()) {
        let mut expected = [0; 13];
        expected[5 + member / 8] = 1 << (member % 8);
        // NO_CLONE: Index a borrowed catalog/map record range without removing it from the
        // captured page inventory used by preservation audit. Only range bounds
        // are copied, not the payload.
        if source::page(after, source::MAP_PAGE)[range.clone()] != expected {
            return Err(AddTableError::Preservation);
        }
    }
    for (at, (original, proposed)) in before.as_bytes().iter().zip(after).enumerate() {
        check(budget, cancelled)?;
        if original != proposed
            && !permitted(at / PAGE_BYTES, at % PAGE_BYTES, source, &maps, &catalog)
        {
            return Err(AddTableError::Preservation);
        }
    }
    // Native recapture independently decodes appended catalog, map ownership,
    // field declarations and primary membership. Original rows are not repacked.
    let image = NativeImage::capture(
        &mut Cursor::new(after),
        before.catalog().catalog().scope(),
        budget,
        cancelled,
    )?;
    let checked = check_table(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        definition.table(),
        budget,
        cancelled,
    )?;
    let schema = checked.schema();
    if schema.definition_page() != role.definition as u32
        || schema.declared_rows() != 0
        || schema.physical_layout().usage != reference(source::MAP_PAGE as u32, role.owned as u8)
        || checked.rows() != 0
        || schema.columns().len() != definition.column_count()
    {
        return Err(AddTableError::Preservation);
    }
    let columns = definition.borrowed_columns(budget, cancelled)?;
    definition::verify(after, role, definition, &columns, budget, cancelled)?;
    let mut fixed_offset = 0;
    let mut variable = 0;
    for (position, (observed, declared)) in schema.columns().iter().zip(&columns).enumerate() {
        check(budget, cancelled)?;
        let physical = observed.physical_field();
        let variable_field = matches!(
            declared.scalar_type(),
            ScalarType::Text | ScalarType::Binary | ScalarType::Decimal
        );
        let expected_offset = if variable_field || declared.scalar_type() == ScalarType::Boolean {
            0
        } else {
            fixed_offset
        };
        let sort = if declared.scalar_type() == ScalarType::Text {
            Some(NativeSortOrder::from_native_bytes([9, 4, 0, 1]))
        } else {
            None
        };
        if physical.fixed == variable_field
            || physical.offset != expected_offset
            || physical.variable != variable
            || observed.text_sort_order() != sort
        {
            return Err(AddTableError::Preservation);
        }
        if variable_field {
            variable += 1;
        } else {
            fixed_offset += width(*declared)?;
        }
        budget
            .charge(
                Resource::Steps,
                observed.name().len().max(declared.name().len()) as u64,
            )
            .map_err(crate::InspectError::from)?;
        if observed.name() != declared.name()
            || observed.scalar_type() != Some(declared.scalar_type())
            || observed.number() != position
            || observed.storage_length() != width(*declared)?
            || observed.flags()
                != if declared.incrementing_autonumber() {
                    7
                } else if observed.fixed() {
                    3
                } else {
                    2
                }
            || observed.extended_flags() != 0
            || observed.decimal_parameters()
                != declared
                    .decimal_constraints()
                    .map(|value| (value.precision(), value.scale()))
        {
            return Err(AddTableError::Preservation);
        }
    }
    if schema.physical_layout().fixed_end != 2 + fixed_offset
        || schema.physical_layout().variable_slots != variable
        || schema.physical_layout().max_columns != columns.len()
        || schema.physical_layout().physical_indexes != usize::from(role.primary.is_some())
        || schema.physical_layout().logical_indexes != usize::from(role.primary.is_some())
    {
        return Err(AddTableError::Preservation);
    }
    let properties = properties::verify(&image, schema, &columns, budget, cancelled)?;
    // NO_CLONE: Index a borrowed catalog/map record range without removing it from the
    // captured page inventory used by preservation audit. Only range bounds are
    // copied, not the payload.
    catalog::verify(
        &source::page(after, source::CATALOG_PAGE)[catalog.records[source.catalog.count].clone()],
        definition.table(),
        role.definition as u32,
        &properties,
        budget,
        cancelled,
    )?;
    budget
        .charge(Resource::Steps, image.catalog().origins().len() as u64)
        .map_err(crate::InspectError::from)?;
    let origin = image
        .catalog()
        .origins()
        .iter()
        .find(|origin| origin.native_id() == role.definition as i32)
        .ok_or(AddTableError::Preservation)?;
    if origin.type_code() != 1
        || origin.flags() != 0
        || origin.parent_id() != 0x0f00_0001
        || origin.page() != source::CATALOG_PAGE as u32
        || origin.slot() as usize != role.catalog
    {
        return Err(AddTableError::Preservation);
    }
    let payloads = image.inspect_object_payloads(origin.object_id(), budget, cancelled)?;
    if [payloads.body(), payloads.module(), payloads.extra()]
        .iter()
        .any(|presence| *presence != PayloadPresence::Absent)
    {
        return Err(AddTableError::Preservation);
    }
    let Some(primary) = role.primary else {
        if !checked.indexes().is_empty() || definition.primary_name().is_some() {
            return Err(AddTableError::Preservation);
        }
        return Ok(());
    };
    let [index] = checked.indexes() else {
        return Err(AddTableError::Preservation);
    };
    let primary_name = definition
        .primary_name()
        .ok_or(AddTableError::Preservation)?;
    budget
        .charge(
            Resource::Steps,
            index.index().name().len().max(primary_name.len()) as u64,
        )
        .map_err(crate::InspectError::from)?;
    if index.status() != IndexCheckStatus::Checked
        || index.index().root_page() != primary.root as u32
        || index.index().usage_map() != reference(source::MAP_PAGE as u32, primary.map as u8)
        || index.index().native_type() != 1
        || index.index().name() != primary_name
        || index.index().flags() != 0x89
        || index.index().columns().len() != 1
        || Some(index.index().columns()[0].number()) != definition.primary_position()
        || index.index().columns()[0].descending()
    {
        return Err(AddTableError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}

fn width(column: CreateColumn<'_>) -> Result<usize, AddTableError> {
    Ok(match column.scalar_type() {
        ScalarType::Boolean => 0,
        ScalarType::Byte => size_of::<u8>(),
        ScalarType::Int16 => size_of::<i16>(),
        ScalarType::Int32 | ScalarType::Single => size_of::<u32>(),
        ScalarType::Int64 | ScalarType::Double | ScalarType::Currency | ScalarType::DateTime => {
            size_of::<u64>()
        }
        ScalarType::Guid => 16,
        ScalarType::Decimal => 17,
        ScalarType::Text => {
            column
                .max_text_utf16_units()
                .ok_or(AddTableError::Preservation)?
                * size_of::<u16>()
        }
        ScalarType::Binary => column
            .max_binary_bytes()
            .ok_or(AddTableError::Preservation)?,
        ScalarType::DateTimeExtended => return Err(AddTableError::Preservation),
    })
}
