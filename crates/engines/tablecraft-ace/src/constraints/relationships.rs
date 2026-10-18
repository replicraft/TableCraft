use super::{ConstraintViolation, RelationshipCheckStatus};
use crate::{
    InspectError, NativeRelationship, TableSnapshot, format::layout::INDEX_KEY_COLUMNS,
    header::check, inspect_table_indexes, read_table,
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
    if declaration.columns().is_empty() || declaration.columns().len() > INDEX_KEY_COLUMNS {
        return Ok(RelationshipCheckStatus::Unsupported);
    }
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
    let mut parent_columns = [0; INDEX_KEY_COLUMNS];
    let mut child_columns = [0; INDEX_KEY_COLUMNS];
    for (i, pair) in declaration.columns().iter().enumerate() {
        check(budget, cancelled)?;
        let Some(parent_column) = position(parent, pair.parent().as_str()) else {
            return Ok(RelationshipCheckStatus::Unsupported);
        };
        let Some(child_column) = position(child, pair.child().as_str()) else {
            return Ok(RelationshipCheckStatus::Unsupported);
        };
        if parent.columns()[parent_column].data_type() != child.columns()[child_column].data_type()
        {
            return Ok(RelationshipCheckStatus::Unsupported);
        }
        parent_columns[i] = parent_column;
        child_columns[i] = child_column;
    }
    let parent_columns = &parent_columns[..declaration.columns().len()];
    let child_columns = &child_columns[..declaration.columns().len()];
    let indexes = inspect_table_indexes(source, scope, parent_name, budget, cancelled)?;
    let eligible = indexes.indexes().iter().any(|index| {
        index.columns().len() == parent_columns.len()
            && index
                .columns()
                .iter()
                .zip(parent_columns)
                .all(|(key, position)| {
                    key.number() == parent.schema().columns()[*position].number()
                })
            && index
                .constraints()
                .is_some_and(|constraints| constraints.primary_key() || constraints.unique())
    });
    if !eligible {
        return Ok(RelationshipCheckStatus::Unsupported);
    }
    check_keys(
        parent,
        parent_columns,
        child,
        child_columns,
        rules.one_to_one,
        budget,
        cancelled,
    )?;
    Ok(RelationshipCheckStatus::Checked)
}

fn position(table: &TableSnapshot, name: &str) -> Option<usize> {
    table.columns().iter().position(|column| {
        column.name().eq_ignore_ascii_case(name)
            && matches!(column.data_type(), ScalarType::Int32 | ScalarType::Int64)
    })
}

fn check_keys(
    parent: &TableSnapshot,
    parent_columns: &[usize],
    child: &TableSnapshot,
    child_columns: &[usize],
    one_to_one: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    let parents = keys(parent, parent_columns, budget, cancelled)?;
    let children = keys(child, child_columns, budget, cancelled)?;
    validate_keys(parents, children, one_to_one, budget, cancelled)
}

fn validate_keys<K: Ord>(
    mut parents: Vec<K>,
    mut children: Vec<K>,
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
    let search_steps =
        (u64::from(parents.len().checked_ilog2().unwrap_or(0)) + 1) * INDEX_KEY_COLUMNS as u64;
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
    columns: &[usize],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<[Option<i64>; INDEX_KEY_COLUMNS]>, InspectError> {
    budget.charge_allocation(
        table.rows().len() as u64,
        size_of::<[Option<i64>; INDEX_KEY_COLUMNS]>() as u64,
    )?;
    let mut keys = Vec::new();
    keys.try_reserve_exact(table.rows().len())
        .map_err(|_| InspectError::Allocation)?;
    for row in table.rows() {
        check(budget, cancelled)?;
        let mut key = [None; INDEX_KEY_COLUMNS];
        let mut present = false;
        for (i, position) in columns.iter().enumerate() {
            check(budget, cancelled)?;
            match row.get(*position) {
                Some(Value::Int32(value)) => {
                    key[i] = Some(i64::from(*value));
                    present = true;
                }
                Some(Value::Int64(value)) => {
                    key[i] = Some(*value);
                    present = true;
                }
                Some(Value::Null) => {}
                _ => return Err(InspectError::Malformed),
            }
        }
        // Entirely NULL keys are exempt; partially NULL keys remain exact tuples.
        if present {
            keys.push(key);
        }
    }
    Ok(keys)
}
fn sort<K: Ord>(
    keys: &mut [K],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    check(budget, cancelled)?;
    let levels = u64::from(keys.len().checked_ilog2().unwrap_or(0)) + 1;
    budget.charge(
        Resource::Steps,
        keys.len() as u64 * levels * INDEX_KEY_COLUMNS as u64,
    )?;
    keys.sort_unstable();
    check(budget, cancelled)
}

#[cfg(test)]
mod tests;
