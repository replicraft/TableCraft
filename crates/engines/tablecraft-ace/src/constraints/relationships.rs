use super::RelationshipCheckStatus;
mod decimal;
mod keys;
mod membership;
use crate::{
    InspectError, NativeRelationship, TableSnapshot, format::layout::INDEX_KEY_COLUMNS,
    header::check, inspect_table_indexes, read_table,
};
use keys::{keys, position};
use membership::validate_keys;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_model::CatalogScope;
use tablecraft_schema::ScalarType;

pub(super) struct RelationshipCoverage {
    pub(super) status: RelationshipCheckStatus,
    pub(super) existing_mutation_key_profile: bool,
}

impl RelationshipCoverage {
    fn unsupported() -> Self {
        Self {
            status: RelationshipCheckStatus::Unsupported,
            existing_mutation_key_profile: false,
        }
    }
}

pub(super) fn check_relationship<R: crate::NativeReadSource>(
    source: &mut R,
    scope: CatalogScope,
    relationship: &NativeRelationship,
    target: &TableSnapshot,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<RelationshipCoverage, InspectError> {
    check(budget, cancelled)?;
    let declaration = relationship.declaration();
    let Some(rules) = declaration.rules() else {
        return Ok(RelationshipCoverage::unsupported());
    };
    if !rules.enforced {
        return Ok(RelationshipCoverage {
            status: RelationshipCheckStatus::NotEnforced,
            existing_mutation_key_profile: true,
        });
    }
    if declaration.columns().is_empty() || declaration.columns().len() > INDEX_KEY_COLUMNS {
        return Ok(RelationshipCoverage::unsupported());
    }
    let parent_name = declaration.parent_table().as_str();
    let child_name = declaration.child_table().as_str();
    let parent = if parent_name.eq_ignore_ascii_case(target.schema().name()) {
        None
    } else {
        match read_table(source, scope, parent_name, budget, cancelled) {
            Ok(table) => Some(table),
            Err(InspectError::UnknownTable | InspectError::Unsupported | InspectError::Encoded) => {
                return Ok(RelationshipCoverage::unsupported());
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
                return Ok(RelationshipCoverage::unsupported());
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
            return Ok(RelationshipCoverage::unsupported());
        };
        let Some(child_column) = position(child, pair.child().as_str()) else {
            return Ok(RelationshipCoverage::unsupported());
        };
        if parent.columns()[parent_column].data_type() != child.columns()[child_column].data_type()
            || !decimal::compatible(
                &parent.schema().columns()[parent_column],
                &child.schema().columns()[child_column],
            )
        {
            return Ok(RelationshipCoverage::unsupported());
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
        return Ok(RelationshipCoverage::unsupported());
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
    // Read support must not implicitly qualify a new mutation key carrier.
    let existing_mutation_key_profile = parent_columns.iter().all(|&position| {
        matches!(
            parent.columns()[position].data_type(),
            ScalarType::Byte
                | ScalarType::Int16
                | ScalarType::Int32
                | ScalarType::Int64
                | ScalarType::Currency
                | ScalarType::Guid
                | ScalarType::Decimal
        )
    });
    Ok(RelationshipCoverage {
        status: RelationshipCheckStatus::Checked,
        existing_mutation_key_profile,
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
    if parent_columns.iter().any(|&position| {
        parent.columns()[position].data_type() == tablecraft_schema::ScalarType::Decimal
    }) {
        return decimal::check_keys(
            parent,
            parent_columns,
            child,
            child_columns,
            one_to_one,
            budget,
            cancelled,
        );
    }
    let parents = keys(parent, parent_columns, budget, cancelled)?;
    let children = keys(child, child_columns, budget, cancelled)?;
    validate_keys(parents, children, one_to_one, budget, cancelled)
}

#[cfg(test)]
mod tests;
