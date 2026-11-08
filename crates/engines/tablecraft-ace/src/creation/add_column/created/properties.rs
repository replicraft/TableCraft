//! Closed inert field-property reconstruction, independent of target authoring.
use super::CreatedAddColumnError;
use crate::{
    EditError, InspectError, PropertyGroupKind, TableProperties, TableSchema, header::check,
    table::mutation::insert_defaults::proof_literal,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

const DDL_FLAGS: u8 = 1;
const BOOLEAN: u8 = 1;
const TEXT: u8 = 10;
const MEMO: u8 = 12;

#[derive(Default)]
pub(super) struct Properties<'a> {
    seen: bool,
    pub(super) required: Option<bool>,
    pub(super) allow_empty: Option<bool>,
    pub(super) default: Option<Value>,
    pub(super) description: Option<&'a str>,
}

pub(super) fn capture<'a>(
    schema: &TableSchema,
    properties: &'a TableProperties,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Properties<'a>>, CreatedAddColumnError> {
    budget
        .charge_allocation(
            schema.columns().len() as u64,
            size_of::<Properties<'_>>() as u64,
        )
        .map_err(InspectError::from)?;
    let mut policies = Vec::new();
    policies
        .try_reserve_exact(schema.columns().len())
        .map_err(|_| InspectError::Allocation)?;
    policies.resize_with(schema.columns().len(), Properties::default);
    for group in properties.groups() {
        check(budget, cancelled)?;
        if group.kind() != PropertyGroupKind::Column {
            return Err(CreatedAddColumnError::Source);
        }
        let mut position = None;
        for (index, field) in schema.columns().iter().enumerate() {
            check(budget, cancelled)?;
            budget
                .charge(
                    Resource::Steps,
                    field.name().len().max(group.name().len()) as u64,
                )
                .map_err(InspectError::from)?;
            if field.name() == group.name() {
                position = Some(index);
                break;
            }
        }
        let position = position.ok_or(CreatedAddColumnError::Source)?;
        let policy = &mut policies[position];
        // Generated fields cannot carry policies, even if their value looks inert.
        if position == 0 || policy.seen {
            return Err(CreatedAddColumnError::Source);
        }
        policy.seen = true;
        for property in group.properties() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, property.name().len() as u64)
                .map_err(InspectError::from)?;
            if property.flags() != DDL_FLAGS {
                return Err(CreatedAddColumnError::Source);
            }
            match property.name() {
                "Required" if policy.required.is_none() && property.native_type() == BOOLEAN => {
                    let value = property.boolean()?;
                    if property.raw_value() != [if value { 255 } else { 0 }] {
                        return Err(CreatedAddColumnError::Source);
                    }
                    policy.required = Some(value);
                }
                "AllowZeroLength"
                    if policy.allow_empty.is_none() && property.native_type() == BOOLEAN =>
                {
                    let value = property.boolean()?;
                    if property.raw_value() != [if value { 255 } else { 0 }] {
                        return Err(CreatedAddColumnError::Source);
                    }
                    policy.allow_empty = Some(value);
                }
                "Description" if policy.description.is_none() && property.native_type() == TEXT => {
                    policy.description = Some(property.text()?);
                }
                "DefaultValue" if policy.default.is_none() && property.native_type() == MEMO => {
                    let mut used_clock = false;
                    policy.default = Some(
                        proof_literal::derive(
                            &schema.columns()[position],
                            property.text()?,
                            None,
                            &mut used_clock,
                            budget,
                            cancelled,
                        )
                        .map_err(|error| match error {
                            EditError::Inspect(error) => CreatedAddColumnError::Inspect(error),
                            _ => CreatedAddColumnError::Source,
                        })?,
                    );
                    if used_clock {
                        return Err(CreatedAddColumnError::Source);
                    }
                }
                _ => return Err(CreatedAddColumnError::Source),
            }
        }
    }
    Ok(policies)
}
