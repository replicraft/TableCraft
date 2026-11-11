//! Qualified stored collections expose owned metadata and history without FileData reads.
mod copy;
mod metadata;
mod scan;

use super::{NativeSource, query_error};
use std::{cell::OnceCell, sync::Arc};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_db::{OwnedRecordset, RecordsetError};
use tablecraft_query::{QueryError, SourceFailure, source::RecordsetColumn};

pub(super) struct Recordsets {
    columns: Vec<RecordsetColumn>,
    groups: Vec<Group>,
}

struct Group {
    owner: usize,
    members: Vec<usize>,
    rows: OnceCell<Vec<Arc<OwnedRecordset>>>,
}

fn check(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), QueryError> {
    if cancelled() {
        return Err(QueryError::Cancelled);
    }
    budget
        .charge(Resource::Steps, 1)
        .map_err(crate::source::query_limit)
}

fn allocate<T>(count: usize, budget: &mut ResourceBudget) -> Result<Vec<T>, QueryError> {
    budget
        .charge_allocation(count as u64, size_of::<T>() as u64)
        .map_err(crate::source::query_limit)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| QueryError::SourceFailure(SourceFailure::Allocation))?;
    Ok(values)
}

fn recordset_error(error: RecordsetError) -> QueryError {
    match error {
        RecordsetError::Limit(error) => crate::source::query_limit(error),
        RecordsetError::Cancelled => QueryError::Cancelled,
        _ => QueryError::Source,
    }
}

#[cfg(test)]
mod tests;
