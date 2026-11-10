//! Resolved complex child references do not grant generic scalar eligibility.
use crate::{
    ComplexColumnSchema, InspectError, ReadProfile, TableSchema,
    format::{
        data_type,
        layout::{AUTO_NUMBER_FLAG, COMPLEX_FK_EXT_FLAG, UPDATABLE_FLAG},
    },
    header::check,
    table::snapshot,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::{Column, ScalarType};

const MODERN_IDENTITY_EXT_FLAG: u8 = 0x04;

pub(super) fn qualify_variable(
    column: &ComplexColumnSchema,
    flat: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    check(budget, cancelled)?;
    budget.charge(Resource::Steps, (flat.columns().len() * 2) as u64)?;
    if !flat.same_definition(column.flat_table()) {
        return Err(InspectError::Malformed);
    }
    let reference = |number| {
        flat.columns()
            .iter()
            .find(|field| field.number() == number)
            .ok_or(InspectError::Malformed)
    };
    let foreign = reference(column.foreign_key_number())?;
    let identity = reference(column.identity_number())?;
    if column.foreign_key_number() == column.identity_number()
        || !matches!(
            foreign.read_profile(),
            ReadProfile::Ace2007
                | ReadProfile::Ace2010
                | ReadProfile::Ace2016
                | ReadProfile::Ace2019
        )
        || foreign.fixed()
        || identity.fixed()
        || foreign.native_type() != data_type::INT32
        || identity.native_type() != data_type::INT32
        || foreign.storage_length() != data_type::INT32_BYTES
        || identity.storage_length() != data_type::INT32_BYTES
        || foreign.flags() != UPDATABLE_FLAG
        || foreign.extended_flags() != COMPLEX_FK_EXT_FLAG
        || identity.flags() != UPDATABLE_FLAG | AUTO_NUMBER_FLAG
        || identity.extended_flags() != MODERN_IDENTITY_EXT_FLAG
    {
        return Err(InspectError::Unsupported);
    }
    Ok(())
}

/// Preserve all original field metadata; override only the two proven reference
/// carriers. Every member payload still needs its ordinary scalar declaration.
pub(super) fn columns(
    column: &ComplexColumnSchema,
    flat: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Column>, InspectError> {
    if !flat.same_definition(column.flat_table()) {
        return Err(InspectError::Malformed);
    }
    let variable = flat.columns().iter().any(|field| {
        (field.number() == column.foreign_key_number()
            || field.number() == column.identity_number())
            && !field.fixed()
    });
    if variable {
        qualify_variable(column, flat, budget, cancelled)?;
    }
    budget.charge_allocation(flat.columns().len() as u64, size_of::<Column>() as u64)?;
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(flat.columns().len())
        .map_err(|_| InspectError::Allocation)?;
    for field in flat.columns() {
        check(budget, cancelled)?;
        let reference = field.number() == column.foreign_key_number()
            || field.number() == column.identity_number();
        let kind = if variable && reference {
            ScalarType::Int32
        } else {
            field.scalar_type().ok_or(InspectError::Unsupported)?
        };
        columns.push(snapshot::projected_column(field, kind, budget)?);
    }
    Ok(columns)
}
