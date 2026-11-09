use super::{FieldRenameError as Error, source::Qualified};
use crate::{
    InspectError, format::page::PAGE_BYTES, header::check, inspect_catalog,
    inspect_table_properties, inspect_table_schema, properties::framing,
};
use std::io::Cursor;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn candidate(
    source: &Qualified,
    after: &[u8],
    new_name: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    check(budget, cancelled)?;
    let before = source.image();
    if after.len() != before.as_bytes().len() {
        return Err(Error::Preservation);
    }
    let observed = &source.observed;
    let spans = [&observed.definition_name, &observed.property_name];
    budget
        .charge(Resource::Steps, (before.as_bytes().len() * 4) as u64)
        .map_err(InspectError::from)?;
    for (offset, (original, replacement)) in before.as_bytes().iter().zip(after).enumerate() {
        if offset.is_multiple_of(PAGE_BYTES) {
            check(budget, cancelled)?;
        }
        if original != replacement && !spans.iter().any(|span| span.contains(&offset)) {
            return Err(Error::Preservation);
        }
    }
    // Read both original and candidate definition entry bytes while preserving observed source
    // metadata. Only range bounds are copied, not the payload.
    let entry = observed.definition_entry.clone();
    budget.charge(Resource::Steps, (entry.len() * 2) as u64)?;
    // Read both original and candidate definition entry bytes while preserving observed source
    // metadata. Only range bounds are copied, not the payload.
    if before.as_bytes().get(entry.clone()) != after.get(entry) {
        return Err(Error::Preservation);
    }
    let scope = before.catalog().catalog().scope();
    let catalog = inspect_catalog(&mut Cursor::new(after), scope, budget, cancelled)?;
    if catalog.header() != before.catalog().header()
        || catalog.origins().len() != before.catalog().origins().len()
    {
        return Err(Error::Preservation);
    }
    for (left, right) in before.catalog().origins().iter().zip(catalog.origins()) {
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
    if schema.object_id() != observed.schema.object_id()
        || schema.name() != observed.schema.name()
        || schema.definition_page() != observed.schema.definition_page()
        || schema.physical_layout() != observed.schema.physical_layout()
        || schema.columns().len() != observed.schema.columns().len()
    {
        return Err(Error::Preservation);
    }
    // Layout's fields have moved into NativeColumn. Compare those actual fields,
    // never the drained layout shell, while keeping the names independently bound.
    for (left, right) in observed.schema.columns().iter().zip(schema.columns()) {
        check(budget, cancelled)?;
        budget.charge(
            Resource::Steps,
            (left.name().len() + right.name().len()) as u64,
        )?;
        let expected = if left.number() == observed.identity.column_number() {
            new_name
        } else {
            left.name()
        };
        if right.name() != expected || !left.physical_field().same_storage(right.physical_field()) {
            return Err(Error::Preservation);
        }
    }
    let properties = inspect_table_properties(
        &mut Cursor::new(after),
        scope,
        schema.name(),
        budget,
        cancelled,
    )?;
    framing::validate(&properties, budget, cancelled)?;
    if properties.object_id() != observed.properties.object_id()
        || properties.name() != observed.properties.name()
        || properties.raw_bytes().len() != observed.properties.raw_bytes().len()
        || properties.groups().len() != observed.properties.groups().len()
    {
        return Err(Error::Preservation);
    }
    for (index, (left, right)) in observed
        .properties
        .groups()
        .iter()
        .zip(properties.groups())
        .enumerate()
    {
        check(budget, cancelled)?;
        budget.charge(
            Resource::Steps,
            (left.name().len() + right.name().len()) as u64,
        )?;
        let expected = if index == observed.group {
            new_name
        } else {
            left.name()
        };
        if left.kind() != right.kind()
            || right.name() != expected
            || left.properties().len() != right.properties().len()
        {
            return Err(Error::Preservation);
        }
        for (old, new) in left.properties().iter().zip(right.properties()) {
            check(budget, cancelled)?;
            budget.charge(
                Resource::Steps,
                (old.name().len()
                    + new.name().len()
                    + old.raw_value().len()
                    + new.raw_value().len()) as u64,
            )?;
            if old.name() != new.name()
                || old.native_type() != new.native_type()
                || old.flags() != new.flags()
                || old.raw_value() != new.raw_value()
            {
                return Err(Error::Preservation);
            }
        }
    }
    check(budget, cancelled)?;
    Ok(())
}
