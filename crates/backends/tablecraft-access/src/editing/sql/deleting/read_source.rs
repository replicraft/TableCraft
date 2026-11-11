//! Explicit predicate-read mode never retries a failed legacy DELETE planner.
use super::{NativeMutationOptions, RecoverableDeleteError, local_delete_table};
use crate::editing::journal::NativeJournalEditor;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_db::DeletableTable;
use tablecraft_query::{
    MutationOptions, PreparedReadSourceDelete, QueryError, ReadSourceDeleteOptions, Source,
    prepare_read_source_delete_with_options,
};
use tablecraft_sql::{
    query::{Query, Statement},
    select::{Expression, FunctionArgument},
};

/// Identify written subquery predicates before acquiring native snapshot resources.
/// This is mode selection only: unsupported subquery forms still enter explicit
/// source preparation and fail there, without retrying the legacy planner.
/// No source is opened or scanned and no mutation authority is produced.
///
/// # Errors
/// Rejects a nonlocal/non-DELETE target, cancellation, allocation or caller work
/// exhaustion. The iterative walk adds no independent expression-depth ceiling.
pub fn delete_uses_read_source(
    query: &Query,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, QueryError> {
    check(budget, cancelled)?;
    local_delete_table(query, true)?;
    let Statement::Delete(delete) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    let mut pending = Vec::new();
    if let Some(value) = &delete.selection {
        push(&mut pending, value, budget, cancelled)?;
    }
    let mut selected = false;
    while let Some(value) = pending.pop() {
        check(budget, cancelled)?;
        match value {
            Expression::Exists(_) | Expression::Subquery(_) => selected = true,
            Expression::InQuery { expression, .. } | Expression::Quantified { expression, .. } => {
                selected = true;
                push(&mut pending, expression, budget, cancelled)?;
            }
            Expression::Unary { expression, .. }
            | Expression::IsNull { expression, .. }
            | Expression::Member { expression, .. } => {
                push(&mut pending, expression, budget, cancelled)?
            }
            Expression::Binary { left, right, .. } => {
                push(&mut pending, right, budget, cancelled)?;
                push(&mut pending, left, budget, cancelled)?;
            }
            Expression::Like {
                expression,
                pattern,
                ..
            } => {
                push(&mut pending, pattern, budget, cancelled)?;
                push(&mut pending, expression, budget, cancelled)?;
            }
            Expression::Between {
                expression,
                first_bound,
                second_bound,
                ..
            } => {
                push(&mut pending, second_bound, budget, cancelled)?;
                push(&mut pending, first_bound, budget, cancelled)?;
                push(&mut pending, expression, budget, cancelled)?;
            }
            Expression::InList {
                expression, values, ..
            } => {
                for value in values.iter().rev() {
                    push(&mut pending, value, budget, cancelled)?;
                }
                push(&mut pending, expression, budget, cancelled)?;
            }
            Expression::Call { arguments, .. } => {
                for argument in arguments.iter().rev() {
                    check(budget, cancelled)?;
                    if let FunctionArgument::Expression(value) = argument {
                        push(&mut pending, value, budget, cancelled)?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(selected)
}

/// Stage predicates against explicit read sources after native snapshot capture
/// released its budget borrow. Finish only after all read callbacks have ended.
/// The editor retains original snapshot identities and native deletion admission;
/// this wrapper grants no joined-write, cascade or constraint bypass.
///
/// # Errors
/// Retains typed shape/source/query errors and rejects a mismatched native target
/// before scans. Backend qualification remains the consuming finish's responsibility.
pub fn prepare_native_read_source_delete(
    editor: NativeJournalEditor,
    query: &Query,
    reads: &dyn Source,
    query_budget: &mut ResourceBudget,
    options: NativeMutationOptions<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<PreparedReadSourceDelete<NativeJournalEditor>, RecoverableDeleteError> {
    let _span = tracing::debug_span!("access.delete.read_source_prepare").entered();
    let table = local_delete_table(query, true)?;
    if !editor.snapshot().name().eq_ignore_ascii_case(table) {
        return Err(QueryError::Source.into());
    }
    let target_snapshot = editor.snapshot().id();
    Ok(prepare_read_source_delete_with_options(
        editor,
        reads,
        query,
        query_budget,
        ReadSourceDeleteOptions {
            target_snapshot,
            mutation: MutationOptions {
                parameters: Some(options.parameters),
                collation: Some(options.collation),
                collation_resolver: Some(options.resolver),
                text_collations: &[],
            },
        },
        cancelled,
    )?)
}

fn check(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), QueryError> {
    if cancelled() {
        return Err(QueryError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}

fn push<'a>(
    pending: &mut Vec<&'a Expression>,
    value: &'a Expression,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryError> {
    check(budget, cancelled)?;
    if pending.len() == pending.capacity() {
        let added = pending.capacity().max(1);
        budget.charge(Resource::Steps, pending.len() as u64)?;
        budget.charge_allocation(added as u64, size_of::<&Expression>() as u64)?;
        pending
            .try_reserve_exact(added)
            .map_err(|_| QueryError::Allocation)?;
    }
    pending.push(value);
    Ok(())
}

#[cfg(test)]
mod tests;
