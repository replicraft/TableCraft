use super::{ConstraintViolation, RelationshipCheckStatus};
use crate::{
    InspectError, NativeRelationship, TableSnapshot, header::check, inspect_table_indexes,
    read_table,
};
use std::io::{Read, Seek};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

pub(super) fn check_relationship<R: Read + Seek>(
    source: &mut R,
    scope: CatalogScope,
    relationship: &NativeRelationship,
    target: &TableSnapshot,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RelationshipCheckStatus, InspectError> {
    check(budget, cancelled)?;
    let declaration = relationship.declaration();
    let Some(rules) = declaration.rules() else {
        return Ok(RelationshipCheckStatus::Unsupported);
    };
    if !rules.enforced {
        return Ok(RelationshipCheckStatus::NotEnforced);
    }
    if declaration.columns().len() != 1 {
        return Ok(RelationshipCheckStatus::Unsupported);
    }
    let pair = &declaration.columns()[0];
    let parent_name = declaration.parent_table().as_str();
    let child_name = declaration.child_table().as_str();
    let parent = if parent_name.eq_ignore_ascii_case(target.schema().name()) {
        None
    } else {
        match read_table(source, scope, parent_name, budget, cancelled) {
            Ok(table) => Some(table),
            Err(InspectError::UnknownTable | InspectError::Unsupported | InspectError::Encoded) => {
                return Ok(RelationshipCheckStatus::Unsupported);
            }
            Err(error) => return Err(error),
        }
    };
    let child = if child_name.eq_ignore_ascii_case(target.schema().name()) {
        None
    } else {
        match read_table(source, scope, child_name, budget, cancelled) {
            Ok(table) => Some(table),
            Err(InspectError::UnknownTable | InspectError::Unsupported | InspectError::Encoded) => {
                return Ok(RelationshipCheckStatus::Unsupported);
            }
            Err(error) => return Err(error),
        }
    };
    let parent = parent.as_ref().unwrap_or(target);
    let child = child.as_ref().unwrap_or(target);
    let Some(parent_column) = position(parent, pair.parent().as_str()) else {
        return Ok(RelationshipCheckStatus::Unsupported);
    };
    let Some(child_column) = position(child, pair.child().as_str()) else {
        return Ok(RelationshipCheckStatus::Unsupported);
    };
    let indexes = inspect_table_indexes(source, scope, parent_name, budget, cancelled)?;
    let native_number = parent.schema().columns()[parent_column].number();
    let eligible = indexes.indexes().iter().any(|index| {
        index.columns().len() == 1
            && index.columns()[0].number() == native_number
            && index
                .constraints()
                .is_some_and(|constraints| constraints.primary_key() || constraints.unique())
    });
    if !eligible {
        return Ok(RelationshipCheckStatus::Unsupported);
    }
    check_keys(
        parent,
        parent_column,
        child,
        child_column,
        rules.one_to_one,
        budget,
        cancelled,
    )?;
    Ok(RelationshipCheckStatus::Checked)
}

fn position(table: &TableSnapshot, name: &str) -> Option<usize> {
    table.columns().iter().position(|column| {
        column.name().eq_ignore_ascii_case(name) && column.data_type() == ScalarType::Int32
    })
}

fn check_keys(
    parent: &TableSnapshot,
    parent_column: usize,
    child: &TableSnapshot,
    child_column: usize,
    one_to_one: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    let parents = keys(parent, parent_column, budget, cancelled)?;
    let children = keys(child, child_column, budget, cancelled)?;
    validate_keys(parents, children, one_to_one, budget, cancelled)
}

fn validate_keys(
    mut parents: Vec<i32>,
    mut children: Vec<i32>,
    one_to_one: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    sort(&mut parents, budget, cancelled)?;
    for pair in parents.windows(2) {
        check(budget, cancelled)?;
        if pair[0] == pair[1] {
            return Err(ConstraintViolation::DuplicateParent.into());
        }
    }
    if one_to_one {
        sort(&mut children, budget, cancelled)?;
        for pair in children.windows(2) {
            check(budget, cancelled)?;
            if pair[0] == pair[1] {
                return Err(ConstraintViolation::DuplicateChild.into());
            }
        }
    }
    let search_steps = u64::from(parents.len().checked_ilog2().unwrap_or(0)) + 1;
    for key in children {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, search_steps)?;
        if parents.binary_search(&key).is_err() {
            return Err(ConstraintViolation::MissingParent.into());
        }
    }
    Ok(())
}

fn keys(
    table: &TableSnapshot,
    column: usize,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<i32>, InspectError> {
    budget.charge_allocation(table.rows().len() as u64, size_of::<i32>() as u64)?;
    let mut keys = Vec::new();
    keys.try_reserve_exact(table.rows().len())
        .map_err(|_| InspectError::Allocation)?;
    for row in table.rows() {
        check(budget, cancelled)?;
        match row.get(column) {
            Some(Value::Int32(key)) => keys.push(*key),
            Some(Value::Null) => {}
            _ => return Err(InspectError::Malformed),
        }
    }
    Ok(keys)
}
fn sort(
    keys: &mut [i32],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    check(budget, cancelled)?;
    let levels = u64::from(keys.len().checked_ilog2().unwrap_or(0)) + 1;
    budget.charge(Resource::Steps, keys.len() as u64 * levels)?;
    keys.sort_unstable();
    check(budget, cancelled)
}

#[cfg(test)]
mod tests;
