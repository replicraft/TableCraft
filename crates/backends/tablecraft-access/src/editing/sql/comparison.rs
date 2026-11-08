//! Query-only field policy facts are aligned to the actual projected snapshot.
use crate::{collation::identity, editing::NativeEditError};
use tablecraft_ace::{InspectError, NativeImage, TableSchema, inspect_table_schema};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_query::source::TextCollationId;
use tablecraft_schema::{Column, ScalarType};

pub(super) fn schema(
    image: &NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<TableSchema, NativeEditError> {
    Ok(inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?)
}

pub(super) fn align(
    schema: &TableSchema,
    columns: &[Column],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Option<TextCollationId>>, NativeEditError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(
            columns.len() as u64,
            size_of::<Option<TextCollationId>>() as u64,
        )
        .map_err(InspectError::from)?;
    let mut policies = Vec::new();
    policies
        .try_reserve_exact(columns.len())
        .map_err(|_| InspectError::Allocation)?;

    for column in columns {
        let mut matched = None;
        for field in schema.columns() {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    (column.name().len() + field.name().len()) as u64,
                )
                .map_err(InspectError::from)?;
            if field.name() == column.name() {
                if matched.is_some() || field.scalar_type() != Some(column.data_type()) {
                    return Err(InspectError::Malformed.into());
                }
                matched = Some(field);
            }
        }
        let field = matched.ok_or(InspectError::Malformed)?;
        let policy = if column.data_type() == ScalarType::Text {
            Some(identity(
                field.text_sort_order().ok_or(InspectError::Malformed)?,
            ))
        } else {
            None
        };
        policies.push(policy);
    }
    check(budget, cancelled)?;
    Ok(policies)
}

fn check(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), InspectError> {
    if cancelled() {
        return Err(InspectError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}

#[cfg(test)]
mod tests;
