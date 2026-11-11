//! Independent decoded field facts shared by original and native-count append.
use super::{AddTableError, width};
use crate::{CreateColumn, InspectError, NativeSortOrder, TableSchema, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::ScalarType;

pub(in crate::creation::add_table) fn verify(
    schema: &TableSchema,
    columns: &[CreateColumn<'_>],
    keyed: bool,
    text_sort_order: Option<NativeSortOrder>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), AddTableError> {
    if schema.columns().len() != columns.len() {
        return Err(AddTableError::Preservation);
    }
    let mut fixed_offset = 0;
    let mut variable = 0;
    for (position, (observed, declared)) in schema.columns().iter().zip(columns).enumerate() {
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
            text_sort_order
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
            .map_err(InspectError::from)?;
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
        || schema.physical_layout().physical_indexes != usize::from(keyed)
        || schema.physical_layout().logical_indexes != usize::from(keyed)
    {
        return Err(AddTableError::Preservation);
    }
    Ok(())
}
