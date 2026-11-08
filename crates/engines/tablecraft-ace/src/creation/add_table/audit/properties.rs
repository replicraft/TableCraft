//! Compare actual native declarations through the independent constant projector.
use super::super::super::defaults::Literal;
use super::super::AddTableError;
use crate::{
    CreateColumn, EditError, NativeImage, PropertyGroupKind, TableProperties, TableSchema,
    header::check, inspect_table_properties, table::mutation::insert_defaults::proof_literal,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const DDL_FLAGS: u8 = 1;
const BOOLEAN: u8 = 1;
const MEMO: u8 = 12;

pub(super) fn verify(
    image: &NativeImage,
    schema: &TableSchema,
    columns: &[CreateColumn<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableProperties, AddTableError> {
    let properties = inspect_table_properties(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        schema.name(),
        budget,
        cancelled,
    )?;
    budget
        .charge(
            Resource::Steps,
            properties.name().len().max(schema.name().len()) as u64,
        )
        .map_err(crate::InspectError::from)?;
    if properties.object_id() != schema.object_id() || properties.name() != schema.name() {
        return Err(AddTableError::Preservation);
    }
    if !properties.raw_bytes().is_empty() {
        crate::properties::framing::validate(&properties, budget, cancelled)?;
    } else if !properties.groups().is_empty() {
        return Err(AddTableError::Preservation);
    }
    let expected_length = expected_length(columns, budget, cancelled)?;
    if properties.raw_bytes().len() != expected_length {
        return Err(AddTableError::Preservation);
    }
    let mut seen = [false; 16];
    for group in properties.groups() {
        check(budget, cancelled)?;
        if group.kind() != PropertyGroupKind::Column {
            return Err(AddTableError::Preservation);
        }
        let mut position = None;
        for (index, column) in columns.iter().enumerate() {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    column.name().len().max(group.name().len()) as u64,
                )
                .map_err(crate::InspectError::from)?;
            if column.name() == group.name() {
                position = Some(index);
                break;
            }
        }
        let position = position.ok_or(AddTableError::Preservation)?;
        if seen[position] {
            return Err(AddTableError::Preservation);
        }
        seen[position] = true;
        let declaration = columns[position];
        let mut required = None;
        let mut allow_empty = None;
        let mut default = None;
        for property in group.properties() {
            check(budget, cancelled)?;
            if property.flags() != DDL_FLAGS {
                return Err(AddTableError::Preservation);
            }
            budget
                .charge(Resource::Steps, (3 * property.name().len()) as u64)
                .map_err(crate::InspectError::from)?;
            match property.name() {
                "Required" if required.is_none() && property.native_type() == BOOLEAN => {
                    let value = property.boolean()?;
                    if property.raw_value() != [if value { 255 } else { 0 }] {
                        return Err(AddTableError::Preservation);
                    }
                    required = Some(value);
                }
                "AllowZeroLength" if allow_empty.is_none() && property.native_type() == BOOLEAN => {
                    let value = property.boolean()?;
                    if property.raw_value() != [if value { 255 } else { 0 }] {
                        return Err(AddTableError::Preservation);
                    }
                    allow_empty = Some(value);
                }
                "DefaultValue" if default.is_none() && property.native_type() == MEMO => {
                    let expected = declaration
                        .default_literal()
                        .ok_or(AddTableError::Preservation)?;
                    let spelling = Literal::capture(expected, budget, cancelled)?;
                    let text = property.text()?;
                    budget
                        .charge(Resource::Steps, text.len() as u64)
                        .map_err(crate::InspectError::from)?;
                    if !text.encode_utf16().eq(spelling.units()) {
                        return Err(AddTableError::Preservation);
                    }
                    let mut used_clock = false;
                    let value = proof_literal::derive(
                        &schema.columns()[position],
                        property.text()?,
                        None,
                        &mut used_clock,
                        budget,
                        cancelled,
                    )
                    .map_err(|error| match error {
                        EditError::Inspect(error) => AddTableError::Inspect(error),
                        _ => AddTableError::Preservation,
                    })?;
                    if used_clock {
                        return Err(AddTableError::Preservation);
                    }
                    default = Some(value);
                }
                _ => return Err(AddTableError::Preservation),
            }
        }
        let value_bytes = match (default.as_ref(), declaration.default_literal()) {
            (Some(Value::Text(actual)), Some(Value::Text(expected))) => {
                actual.len().max(expected.len())
            }
            _ => size_of::<i128>() + size_of::<u8>(),
        };
        budget
            .charge(Resource::Steps, value_bytes as u64)
            .map_err(crate::InspectError::from)?;
        if required != declaration.required()
            || allow_empty != declaration.allow_zero_length()
            || default.as_ref() != declaration.default_literal()
        {
            return Err(AddTableError::Preservation);
        }
    }
    for (position, column) in columns.iter().enumerate() {
        check(budget, cancelled)?;
        let expected = column.required().is_some()
            || column.allow_zero_length().is_some()
            || column.default_literal().is_some();
        if seen[position] != expected {
            return Err(AddTableError::Preservation);
        }
    }
    Ok(properties)
}

fn expected_length(
    columns: &[CreateColumn<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<usize, AddTableError> {
    check(budget, cancelled)?;
    budget
        .charge(Resource::Steps, (3 * columns.len()) as u64)
        .map_err(crate::InspectError::from)?;
    let required = columns.iter().any(|column| column.required().is_some());
    let allow_empty = columns
        .iter()
        .any(|column| column.allow_zero_length().is_some());
    let default = columns
        .iter()
        .any(|column| column.default_literal().is_some());
    if !required && !allow_empty && !default {
        return Ok(0);
    }
    // MR2 signature and one dictionary block. Names are length-prefixed UTF16.
    let mut length = 4 + 6;
    for (key, present) in [
        ("Required", required),
        ("AllowZeroLength", allow_empty),
        ("DefaultValue", default),
    ] {
        if present {
            budget
                .charge(Resource::Steps, key.len() as u64)
                .map_err(crate::InspectError::from)?;
            length += 2 + key.len() * 2;
        }
    }
    for column in columns {
        check(budget, cancelled)?;
        let booleans = usize::from(column.required().is_some())
            + usize::from(column.allow_zero_length().is_some());
        let literal = column
            .default_literal()
            .map(|value| Literal::capture(value, budget, cancelled))
            .transpose()?;
        if booleans == 0 && literal.is_none() {
            continue;
        }
        budget
            .charge(Resource::Steps, column.name().len() as u64)
            .map_err(crate::InspectError::from)?;
        length += 6 + 4 + 2 + column.name().encode_utf16().count() * 2 + booleans * 9;
        if let Some(literal) = literal {
            length += 8 + literal.payload_bytes();
        }
    }
    Ok(length)
}
