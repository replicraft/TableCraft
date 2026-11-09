//! Prove actual complete canonical generated-primary sources, not creator history.
use super::super::{super::profile, common};
use super::CreatedAddColumnError;
use crate::creation::existing_fields as declarations;
use crate::{
    FileFormat, FirstTableDefinition, IndexCheckStatus, NativeImage, TableCheck, check_table,
    create_ace16_database, header::check,
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_schema::ScalarType;

pub(super) fn capture(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(TableCheck, FirstTableDefinition), CreatedAddColumnError> {
    let (checked, definition) = inspect(image, table, budget, cancelled)?;
    let schema = checked.schema();
    let columns = definition.borrowed_columns(budget, cancelled)?;
    let original = create_ace16_database(schema.name(), &columns, &[], budget, cancelled)?;
    if !common::equal_image(image.as_bytes(), original.as_bytes(), budget, cancelled)? {
        return Err(CreatedAddColumnError::Source);
    }
    drop(columns);
    Ok((checked, definition))
}

/// Shared geometry/declaration observation; only callers prove their full image.
pub(super) fn inspect(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(TableCheck, FirstTableDefinition), CreatedAddColumnError> {
    let checked = geometry(image, table, budget, cancelled)?;
    let definition = declarations::capture(image, &checked, budget, cancelled)?;
    Ok((checked, definition))
}

/// New field-addition observation; the caller must prove the complete source.
pub(super) fn inspect_field_addition(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(TableCheck, FirstTableDefinition), CreatedAddColumnError> {
    let checked = geometry(image, table, budget, cancelled)?;
    let definition = declarations::capture_field_addition(image, &checked, budget, cancelled)?;
    Ok((checked, definition))
}

fn geometry(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableCheck, CreatedAddColumnError> {
    check(budget, cancelled)?;
    image.require_standard_capture()?;
    if image.catalog().header().format() != FileFormat::Ace2016 {
        return Err(CreatedAddColumnError::Source);
    }
    let scope = image.catalog().catalog().scope();
    let checked = check_table(&mut image.reader(), scope, table, budget, cancelled)?;
    let schema = checked.schema();
    if checked.rows() != 0
        || schema.declared_rows() != 0
        || schema.definition_page() != profile::USER_DEFINITION as u32
        || !(1..profile::MAX_COLUMNS).contains(&schema.columns().len())
        || checked.indexes().len() != 1
        || checked.indexes()[0].status() != IndexCheckStatus::Checked
        || schema.columns()[0].scalar_type() != Some(ScalarType::Int32)
    {
        return Err(CreatedAddColumnError::Source);
    }
    let primary = checked.indexes()[0].index();
    if primary.native_type() != 1
        || primary.columns().len() != 1
        || primary.columns()[0].number() != 0
        || primary.columns()[0].descending()
    {
        return Err(CreatedAddColumnError::Source);
    }
    Ok(checked)
}
