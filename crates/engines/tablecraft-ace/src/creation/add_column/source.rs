//! Actual canonical source roles, without literal user names or creator flags.
use super::super::{create_ace16_int32_database, profile};
use super::AddColumnError;
use crate::{
    FileFormat, InspectError, NativeImage, TableCheck, TableSchema, check_table, format::layout,
    header::check,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

const COMPARE_CHUNK_BYTES: usize = 16 * 1024;

pub(super) fn names<'a>(
    schema: &'a TableSchema,
    budget: &mut ResourceBudget,
) -> Result<Vec<&'a str>, InspectError> {
    budget.charge_allocation(
        (schema.columns().len() + 1) as u64,
        size_of::<&str>() as u64,
    )?;
    let mut names = Vec::new();
    names
        .try_reserve_exact(schema.columns().len() + 1)
        .map_err(|_| InspectError::Allocation)?;
    names.extend(schema.columns().iter().map(|field| field.name()));
    Ok(names)
}

pub(super) fn capture(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableCheck, AddColumnError> {
    check(budget, cancelled)?;
    image.require_standard_capture()?;
    if image.catalog().header().format() != FileFormat::Ace2016 {
        return Err(AddColumnError::Source);
    }
    let scope = image.catalog().catalog().scope();
    let checked = check_table(&mut image.reader(), scope, table, budget, cancelled)?;
    let schema = checked.schema();
    let count = schema.columns().len();
    if checked.rows() != 0
        || schema.declared_rows() != 0
        || schema.definition_page() != profile::USER_DEFINITION as u32
        || !checked.indexes().is_empty()
        || !(1..profile::MAX_COLUMNS).contains(&count)
        || schema.columns().iter().any(|field| {
            field.scalar_type() != Some(ScalarType::Int32)
                || field.flags() != (layout::FIXED_FLAG | layout::UPDATABLE_FLAG)
                || field.extended_flags() != 0
                || field.storage_length() != size_of::<i32>()
        })
    {
        return Err(AddColumnError::Source);
    }
    let names = names(schema, budget)?;
    let original = create_ace16_int32_database(schema.name(), &names, &[], budget, cancelled)?;
    // Full structural equality closes catalog/program/allocation and inactive
    // tails. It uses decoded names, never a source hash or remembered creator.
    if image.as_bytes().len() != original.as_bytes().len() {
        return Err(AddColumnError::Source);
    }
    for (actual, expected) in image
        .as_bytes()
        .chunks(COMPARE_CHUNK_BYTES)
        .zip(original.as_bytes().chunks(COMPARE_CHUNK_BYTES))
    {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, actual.len() as u64)
            .map_err(InspectError::from)?;
        if actual != expected {
            return Err(AddColumnError::Source);
        }
    }
    Ok(checked)
}
