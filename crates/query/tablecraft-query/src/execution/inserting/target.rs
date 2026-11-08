//! Shared destination identity and positional mapping for VALUES and SELECT.
use super::values::check;
use crate::{QueryError, profile::SOURCE_SCHEMA_POLICY};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_db::InsertableTable;
use tablecraft_schema::NameComparison;
use tablecraft_sql::query::Insert;

pub(super) fn mapping<E: InsertableTable>(
    editor: &E,
    insert: &Insert,
    input_width: usize,
    allow_omissions: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<usize>, QueryError> {
    let target = insert.target.as_str();
    let policy = editor.schema_policy();
    let columns = editor.columns();
    if editor.name().len() > SOURCE_SCHEMA_POLICY.max_name_bytes {
        return Err(QueryError::StructureLimit);
    }
    if !equal_name(policy.names, target, editor.name(), budget, cancelled)? {
        return Err(QueryError::UnknownName);
    }

    if columns.is_empty()
        || columns.len() > SOURCE_SCHEMA_POLICY.max_columns
        || columns.len() > policy.max_columns
    {
        return Err(QueryError::Width);
    }

    // Bound and validate every identity before any quadratic comparison work.
    // Column constructors already establish the shared nonempty/control-free law.
    for column in columns {
        check(budget, cancelled)?;
        policy
            .validate_name(column.schema_name())
            .map_err(QueryError::from)?;
        SOURCE_SCHEMA_POLICY
            .validate_name(column.schema_name())
            .map_err(QueryError::from)?;
    }

    for (position, column) in columns.iter().enumerate() {
        for prior in &columns[..position] {
            if equal_name(policy.names, prior.name(), column.name(), budget, cancelled)? {
                return Err(QueryError::AmbiguousName);
            }
        }
    }

    if input_width > columns.len()
        || ((!allow_omissions || insert.fields.is_empty()) && input_width != columns.len())
    {
        return Err(QueryError::Width);
    }

    budget
        .charge_allocation(columns.len() as u64, size_of::<usize>() as u64)
        .map_err(QueryError::from)?;
    let mut mapping = Vec::new();
    mapping
        .try_reserve_exact(columns.len())
        .map_err(|_| QueryError::Allocation)?;
    for position in 0..input_width {
        check(budget, cancelled)?;
        let target = if insert.fields.is_empty() {
            position
        } else {
            let mut found = None;
            for (candidate, column) in columns.iter().enumerate() {
                if equal_name(
                    policy.names,
                    column.name(),
                    insert.fields[position].as_str(),
                    budget,
                    cancelled,
                )? {
                    found = Some(candidate);
                    break;
                }
            }
            found.ok_or(QueryError::UnknownName)?
        };
        budget
            .charge(Resource::Steps, mapping.len() as u64)
            .map_err(QueryError::from)?;
        if mapping.contains(&target) {
            return Err(QueryError::DuplicateInsertField);
        }
        mapping.push(target);
    }

    Ok(mapping)
}

fn equal_name(
    policy: NameComparison,
    left: &str,
    right: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, QueryError> {
    check(budget, cancelled)?;

    // Equality first checks width, then at most this many UTF-8 bytes. Pay before
    // the comparison, including for duplicate schema and reordered field scans.
    budget.charge(Resource::Steps, left.len().min(right.len()) as u64)?;
    Ok(policy.equals(left, right))
}
