//! Actual written table-occurrence resolution shared by joined mutation adapters.
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_query::QueryError;
use tablecraft_sql::select::{Table, TableSource};

pub(super) struct Target<'a> {
    pub table: &'a Table,
    pub joined: bool,
}

pub(super) fn find<'a>(
    sources: &'a [TableSource],
    qualifier: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Target<'a>, QueryError> {
    let mut pending = Vec::new();
    for source in sources.iter().rev() {
        push(&mut pending, source, budget, cancelled)?;
    }
    let mut selected = None;
    let mut joined = false;
    while let Some(source) = pending.pop() {
        check(budget, cancelled)?;
        match source {
            TableSource::Table(table) => {
                if table.database.is_some() {
                    return Err(QueryError::Unsupported);
                }
                let alias = table.alias.as_ref().unwrap_or(&table.name).as_str();
                budget.charge(Resource::Steps, alias.len().max(qualifier.len()) as u64)?;
                if alias.eq_ignore_ascii_case(qualifier) && selected.replace(table).is_some() {
                    return Err(QueryError::AmbiguousName);
                }
            }
            TableSource::Join { left, right, .. } => {
                joined = true;
                push(&mut pending, right.as_ref(), budget, cancelled)?;
                push(&mut pending, left.as_ref(), budget, cancelled)?;
            }
            TableSource::Group {
                source,
                alias: None,
            } => {
                push(&mut pending, source.as_ref(), budget, cancelled)?;
            }
            _ => return Err(QueryError::Unsupported),
        }
    }
    Ok(Target {
        table: selected.ok_or(QueryError::UnknownName)?,
        joined,
    })
}

fn check(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), QueryError> {
    if cancelled() {
        return Err(QueryError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}

fn push<'a>(
    pending: &mut Vec<&'a TableSource>,
    source: &'a TableSource,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryError> {
    check(budget, cancelled)?;
    if pending.len() == pending.capacity() {
        let added = pending.capacity().max(1);
        budget.charge(Resource::Steps, pending.len() as u64)?;
        budget.charge_allocation(added as u64, size_of::<&TableSource>() as u64)?;
        pending
            .try_reserve_exact(added)
            .map_err(|_| QueryError::Allocation)?;
    }
    pending.push(source);
    Ok(())
}
