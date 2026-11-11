//! Independent physical complement and decoded-state checks for metadata ADD.
use super::{CurrentFieldAdditionError as Error, Target, definition::Plan};
use crate::{
    InspectError, PropertyGroupKind, TableProperties,
    format::{
        bytes::{u16_at, u32_at},
        data_type::{BOOLEAN, INT32, LONG_TEXT, SHORT_TEXT},
        definition::{LENGTH_OFFSET, PAGE_HEADER_BYTES},
        layout::{
            self, COLUMN_COUNT_OFFSET, COLUMN_ENTRY_BYTES, FIXED_FLAG, MAX_COLUMN_COUNT_OFFSET,
            UNICODE_COMPRESSION_FLAG, UPDATABLE_FLAG, VARIABLE_SLOT_COUNT_OFFSET,
        },
        page::PAGE_BYTES,
        reader::{Reader, row_count, row_range},
        row::{reference_page, reference_slot, slot_offset},
    },
    header::check,
    properties::framing,
    read_table,
    table::mutation::{catalog_property::single_descriptor::DescriptorPatch, current_schema},
};
use std::{io::Cursor, ops::Range};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const FREE_OFFSET: usize = 2;
const ROW_COUNT_OFFSET: usize = 12;
const NAME_LENGTH_BYTES: usize = size_of::<u16>();
const DDL_FLAG: u8 = 1;

#[expect(
    clippy::too_many_arguments,
    reason = "Keep original source, declaration, physical append and proof pools explicit."
)]
pub(super) fn candidate(
    proof: &current_schema::Source,
    plan: &Plan,
    target: Target,
    descriptor: &DescriptorPatch,
    after: &[u8],
    name: &str,
    body: &[u8],
    new_reference: u32,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    check(budget, cancelled)?;
    let before = proof.image();
    let table = proof.table();
    if before.as_bytes().len() != after.len() {
        return Err(Error::Preservation);
    }

    let property_page = reference_page(new_reference) as usize;
    if table.family == current_schema::Family::Current
        && property_page != reference_page(table.database_reference) as usize
    {
        return Err(Error::Preservation);
    }
    let property_base = property_page * PAGE_BYTES;
    let original_page = &before.as_bytes()[property_base..property_base + PAGE_BYTES];
    let new_page = &after[property_base..property_base + PAGE_BYTES];
    let old_count = row_count(original_page)?;
    let new_count = row_count(new_page)?;
    if new_count != old_count + 1 || usize::from(reference_slot(new_reference)) != old_count {
        return Err(Error::Preservation);
    }
    let old_end = if old_count == 0 {
        PAGE_BYTES
    } else {
        row_range(original_page, old_count - 1)?.1
    };
    let (word, new_start, new_end) = row_range(new_page, old_count)?;
    if usize::from(word) != new_start
        || new_end != old_end
        || new_end.checked_sub(new_start) != Some(body.len())
        || &new_page[new_start..new_end] != body
        || usize::from(u16_at(new_page, FREE_OFFSET)?) != new_start - slot_offset(new_count)
    {
        return Err(Error::Preservation);
    }

    let name_frame = NAME_LENGTH_BYTES + name.len() * size_of::<u16>();
    let delta = COLUMN_ENTRY_BYTES + name_frame;
    let new_length = plan.length + delta;
    let definition = plan.base;
    let permitted = [
        definition + FREE_OFFSET..definition + FREE_OFFSET + size_of::<u16>(),
        definition + LENGTH_OFFSET..definition + LENGTH_OFFSET + size_of::<u32>(),
        definition + MAX_COLUMN_COUNT_OFFSET
            ..definition + MAX_COLUMN_COUNT_OFFSET + size_of::<u16>(),
        definition + COLUMN_COUNT_OFFSET..definition + COLUMN_COUNT_OFFSET + size_of::<u16>(),
        definition + VARIABLE_SLOT_COUNT_OFFSET
            ..definition + VARIABLE_SLOT_COUNT_OFFSET + size_of::<u16>(),
        definition + plan.descriptor_end..definition + new_length,
        property_base + FREE_OFFSET..property_base + FREE_OFFSET + size_of::<u16>(),
        property_base + ROW_COUNT_OFFSET..property_base + ROW_COUNT_OFFSET + size_of::<u16>(),
        property_base + slot_offset(old_count)..property_base + slot_offset(new_count),
        property_base + new_start..property_base + new_end,
        descriptor.page_range(),
    ];
    let comparisons = (after.len() as u64)
        .checked_mul(permitted.len() as u64)
        .ok_or(InspectError::StructureLimit)?;
    budget.charge(Resource::Steps, comparisons)?;
    for (at, (old, new)) in before.as_bytes().iter().zip(after).enumerate() {
        if at.is_multiple_of(PAGE_BYTES) {
            check(budget, cancelled)?;
        }
        if old != new && !permitted.iter().any(|range| range.contains(&at)) {
            return Err(Error::Preservation);
        }
    }
    descriptor.verify(after)?;

    let old_definition = &before.as_bytes()[definition..definition + PAGE_BYTES];
    let new_definition = &after[definition..definition + PAGE_BYTES];
    if u32_at(new_definition, LENGTH_OFFSET)? as usize != new_length
        || usize::from(u16_at(new_definition, FREE_OFFSET)?)
            != PAGE_BYTES - new_length - PAGE_HEADER_BYTES
        || usize::from(u16_at(new_definition, MAX_COLUMN_COUNT_OFFSET)?) != plan.number + 1
        || usize::from(u16_at(new_definition, COLUMN_COUNT_OFFSET)?) != plan.active_count + 1
        || usize::from(u16_at(new_definition, VARIABLE_SLOT_COUNT_OFFSET)?)
            != plan.variable_high_water + usize::from(variable_target(target)?)
        || old_definition[plan.descriptor_start..plan.descriptor_end]
            != new_definition[plan.descriptor_start..plan.descriptor_end]
        || old_definition[plan.descriptor_end..plan.index_start]
            != new_definition
                [plan.descriptor_end + COLUMN_ENTRY_BYTES..plan.index_start + COLUMN_ENTRY_BYTES]
        || old_definition[plan.index_start..plan.length]
            != new_definition[plan.index_start + delta..new_length]
    {
        return Err(Error::Preservation);
    }

    metadata(proof, plan, target, after, name, body, budget, cancelled)
}

#[expect(
    clippy::too_many_arguments,
    reason = "Compare the original source, exact typed append and independent retained metadata."
)]
fn metadata(
    proof: &current_schema::Source,
    plan: &Plan,
    target: Target,
    after: &[u8],
    name: &str,
    body: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    let original = proof.table();
    let before = proof.image();
    let scope = before.catalog().catalog().scope();
    let current = original.family.image(after, scope, budget, cancelled)?;
    let catalog = current.catalog();
    if catalog.header() != before.catalog().header()
        || catalog.origins().len() != before.catalog().origins().len()
    {
        return Err(Error::Preservation);
    }
    for (old, new) in before.catalog().origins().iter().zip(catalog.origins()) {
        check(budget, cancelled)?;
        if old.object_id() != new.object_id()
            || old.native_id() != new.native_id()
            || old.parent_id() != new.parent_id()
            || old.type_code() != new.type_code()
            || old.flags() != new.flags()
            || old.page() != new.page()
            || old.slot() != new.slot()
        {
            return Err(Error::Preservation);
        }
    }

    // Requalify actual replacement metadata/program/ownership facts rather
    // than trusting descriptor bytes emitted by the append implementation.
    let checked = original
        .family
        .observe(&current, original.schema.name(), budget, cancelled)?;
    let schema = &checked.schema;
    if schema.object_id() != original.schema.object_id()
        || schema.definition_page() != original.schema.definition_page()
        || schema.declared_rows() != original.schema.declared_rows()
        || schema.columns().len() != original.schema.columns().len() + 1
    {
        return Err(Error::Preservation);
    }
    for (old, new) in original.schema.columns().iter().zip(schema.columns()) {
        check(budget, cancelled)?;
        if old.name() != new.name() || !old.physical_field().same_storage(new.physical_field()) {
            return Err(Error::Preservation);
        }
    }
    let field = schema.columns().last().ok_or(Error::Preservation)?;
    if field.name() != name
        || field.number() != plan.number
        || field.physical_field().variable != plan.variable
    {
        return Err(Error::Preservation);
    }
    let declaration_matches = match target {
        Target::Typed(target) => {
            let expected = target
                .field(plan.fixed_offset)
                .map_err(super::creation_error)?;
            field.native_type() == expected.kind
                && field.fixed() == expected.offset.is_some()
                && field.storage_length() == expected.width
                && field.flags()
                    == (UPDATABLE_FLAG
                        | if expected.offset.is_some() {
                            FIXED_FLAG
                        } else {
                            0
                        })
                && field.extended_flags()
                    == if expected.unicode_compression {
                        UNICODE_COMPRESSION_FLAG
                    } else {
                        0
                    }
                && field.physical_field().offset == expected.offset.unwrap_or(0)
                && field.decimal_parameters()
                    == expected
                        .decimal
                        .map(|constraints| (constraints.precision(), constraints.scale()))
                && (expected.kind != SHORT_TEXT
                    || field.text_sort_order().is_some_and(|order| {
                        order.general_family() == Some(crate::GeneralSortFamily::General2010)
                    }))
        }
        Target::Number => {
            field.native_type() == INT32
                && field.fixed()
                && field.storage_length() == size_of::<i32>()
                && field.flags() == (FIXED_FLAG | UPDATABLE_FLAG)
                && field.extended_flags() == 0
                && field.physical_field().offset == plan.fixed_offset
        }
        Target::ShortText {
            max_utf16_units,
            unicode_compression,
        } => {
            field.native_type() == SHORT_TEXT
                && !field.fixed()
                && field.storage_length() == max_utf16_units * size_of::<u16>()
                && field.flags() == UPDATABLE_FLAG
                && field.extended_flags()
                    == if unicode_compression {
                        UNICODE_COMPRESSION_FLAG
                    } else {
                        0
                    }
                && field.physical_field().offset == 0
                && field.text_sort_order().is_some_and(|order| {
                    order.general_family() == Some(crate::GeneralSortFamily::General2010)
                })
        }
    };
    if !declaration_matches {
        return Err(Error::Preservation);
    }

    let mut input = current.reader();
    let mut reader = Reader {
        source: &mut input,
        header: current.catalog().header(),
        budget,
        cancelled,
        reads: 0,
    };
    let mut stored_id = None;
    layout::read_with_field_names(&mut reader, schema.definition_page(), |number, id, _, _| {
        if number == plan.number && stored_id.replace(id).is_some() {
            return Err(InspectError::Malformed);
        }
        Ok(())
    })?;
    if stored_id != Some(plan.column_id) {
        return Err(Error::Preservation);
    }

    let properties = &checked.properties;
    framing::current_schema_names(properties, budget, cancelled)?;
    if properties.raw_bytes() != body
        || properties.groups().len() != original.properties.groups().len() + 1
    {
        return Err(Error::Preservation);
    }

    let old_names = if original.properties.raw_bytes().is_empty() {
        Vec::new()
    } else {
        framing::current_schema_names(&original.properties, budget, cancelled)?
    };
    let new_names = framing::current_schema_names(properties, budget, cancelled)?;
    if new_names.len() < old_names.len()
        || !old_names
            .iter()
            .zip(&new_names)
            .all(|(old, new)| old == new)
    {
        return Err(Error::Preservation);
    }

    let mut old_blocks = Vec::new();
    let mut new_blocks = Vec::new();
    blocks(&original.properties, &mut old_blocks, budget, cancelled)?;
    blocks(properties, &mut new_blocks, budget, cancelled)?;
    for (old, new) in old_blocks.into_iter().zip(new_blocks) {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, (old.len() + new.len()) as u64)?;
        if original.properties.raw_bytes()[old] != properties.raw_bytes()[new] {
            return Err(Error::Preservation);
        }
    }

    let group = properties.groups().last().ok_or(Error::Preservation)?;
    if group.kind() != PropertyGroupKind::Column
        || group.name() != name
        || group.properties().len() != target.property_keys().len()
    {
        return Err(Error::Preservation);
    }
    if let Target::Typed(target) = target {
        let literal = target
            .literal(budget, cancelled)
            .map_err(super::creation_error)?;
        for property in group.properties() {
            if property.flags() != DDL_FLAG {
                return Err(Error::Preservation);
            }
            let matches = match property.name() {
                "Required" => property.native_type() == BOOLEAN && property.raw_value() == [0],
                "AllowZeroLength"
                    if target.scalar_type() == tablecraft_schema::ScalarType::Text =>
                {
                    property.native_type() == BOOLEAN && property.raw_value() == [255]
                }
                "DefaultValue" => literal.as_ref().is_some_and(|literal| {
                    property.native_type() == LONG_TEXT
                        && property.raw_value().len() == literal.payload_bytes()
                        && property
                            .raw_value()
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .copied()
                            .map(u16::from_le_bytes)
                            .eq(literal.units())
                }),
                _ => false,
            };
            if !matches {
                return Err(Error::Preservation);
            }
        }
    } else {
        let mut required = false;
        let mut second = false;
        for property in group.properties() {
            if property.flags() != DDL_FLAG {
                return Err(Error::Preservation);
            }
            if property.name().eq_ignore_ascii_case("Required")
                && !required
                && property.native_type() == BOOLEAN
                && property.raw_value() == [0]
            {
                required = true;
            } else if !second
                && match target {
                    Target::Typed(_) => false,
                    Target::Number => {
                        property.name().eq_ignore_ascii_case("DefaultValue")
                            && property.native_type() == LONG_TEXT
                            && property.raw_value() == [b'0', 0]
                    }
                    Target::ShortText { .. } => {
                        property.name().eq_ignore_ascii_case("AllowZeroLength")
                            && property.native_type() == BOOLEAN
                            && property.raw_value() == [255]
                    }
                }
            {
                second = true;
            } else {
                return Err(Error::Preservation);
            }
        }
        if !required || !second {
            return Err(Error::Preservation);
        }
    }

    let database = &checked.database;
    if database.object_id() != original.database.object_id()
        || database.raw_bytes() != original.database.raw_bytes()
    {
        return Err(Error::Preservation);
    }

    let old_rows = read_table(
        &mut before.reader(),
        scope,
        schema.name(),
        budget,
        cancelled,
    )?;
    let new_rows = read_table(
        &mut Cursor::new(after),
        scope,
        schema.name(),
        budget,
        cancelled,
    )?;
    if old_rows.rows().len() != new_rows.rows().len() {
        return Err(Error::Preservation);
    }
    for (old, new) in old_rows.rows().iter().zip(new_rows.rows()) {
        check(budget, cancelled)?;
        if new.len() != old.len() + 1
            || &new[..old.len()] != old
            || new.last()
                != Some(
                    &if matches!(target, Target::Typed(target) if target.scalar_type() == tablecraft_schema::ScalarType::Boolean)
                    {
                        Value::Boolean(false)
                    } else {
                        Value::Null
                    },
                )
        {
            return Err(Error::Preservation);
        }
    }

    Ok(())
}

fn blocks(
    properties: &TableProperties,
    output: &mut Vec<Range<usize>>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    if properties.raw_bytes().is_empty() {
        if !properties.groups().is_empty() {
            return Err(Error::Preservation);
        }
        return Ok(());
    }

    budget.charge_allocation(
        properties.groups().len() as u64,
        size_of::<Range<usize>>() as u64,
    )?;
    output
        .try_reserve_exact(properties.groups().len())
        .map_err(|_| InspectError::Allocation)?;
    framing::current_schema_group_blocks(properties, budget, cancelled, |_, block| {
        output.push(block);

        Ok(())
    })?;

    Ok(())
}

fn variable_target(target: Target) -> Result<bool, Error> {
    match target {
        Target::Number => Ok(false),
        Target::ShortText { .. } => Ok(true),
        Target::Typed(target) => Ok(target
            .field(0)
            .map_err(super::creation_error)?
            .offset
            .is_none()),
    }
}
