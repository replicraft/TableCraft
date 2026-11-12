//! Explicit DISTINCTROW joins retain original target identities and TCND admission.
use super::{NativeMutationOptions, RecoverableDeleteError};
use crate::editing::{journal::NativeJournalEditor, sql::joined_target};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_db::DeletableTable;
use tablecraft_query::{
    JoinedMutationOptions, MutationOptions, PreparedJoinedDelete, QueryError, Source,
    prepare_joined_delete_with_options,
};
use tablecraft_sql::{
    query::{Query, Statement},
    select::TableSource,
};

/// One written local target occurrence; no native or persistence authority.
#[derive(Clone, Copy)]
pub struct JoinedDeleteTarget<'a> {
    /// Actual native source table name, independent from the written alias.
    pub table: &'a str,
    /// Exact qualifier selecting the original target occurrence in the join.
    pub qualifier: &'a str,
}

/// Resolve one affirmative DISTINCTROW target from a local explicit JOIN tree.
/// This syntax-only operation performs no source lookup or native record reads.
/// Comma products, multiple targets and external/derived sources remain refused.
///
/// # Errors
/// Rejects unsupported/missing/ambiguous target syntax, cancellation, allocation
/// failure and caller work exhaustion. No failed local planner is retried here.
pub fn local_joined_delete_target<'a>(
    query: &'a Query,
    allow_parameters: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<JoinedDeleteTarget<'a>, QueryError> {
    if cancelled() {
        return Err(QueryError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    let Statement::Delete(delete) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    let ([qualifier], [_]) = (delete.targets.as_slice(), delete.sources.as_slice()) else {
        return Err(QueryError::Unsupported);
    };
    if !delete.distinct_row
        || (!allow_parameters && !query.parameters.is_empty())
        || matches!(delete.sources.as_slice(), [TableSource::Table(_)])
    {
        return Err(QueryError::Unsupported);
    }
    let selected = joined_target::find(&delete.sources, qualifier.as_str(), budget, cancelled)?;
    if !selected.joined {
        return Err(QueryError::Unsupported);
    }
    Ok(JoinedDeleteTarget {
        table: selected.table.name.as_str(),
        qualifier: selected
            .table
            .alias
            .as_ref()
            .unwrap_or(&selected.table.name)
            .as_str(),
    })
}

/// Stage all joined selection using the captured target's original snapshot.
/// The caller must release native pool borrows before this call and reacquire
/// them only for consuming finish. Repeated matches identify one original row;
/// they never bypass eager errors or native constraints/program/deletion proof.
///
/// # Errors
/// Retains typed syntax/source/query failures and refuses a mismatched editor
/// before scans. Native write admission remains the unchanged consuming finish.
pub fn prepare_native_joined_delete(
    editor: NativeJournalEditor,
    query: &Query,
    reads: &dyn Source,
    query_budget: &mut ResourceBudget,
    options: NativeMutationOptions<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<PreparedJoinedDelete<NativeJournalEditor>, RecoverableDeleteError> {
    let _span = tracing::debug_span!("access.joined_delete.prepare").entered();
    let target = local_joined_delete_target(query, true, query_budget, cancelled)?;
    if !editor.snapshot().name().eq_ignore_ascii_case(target.table) {
        return Err(QueryError::Source.into());
    }
    let target_snapshot = editor.snapshot().id();
    Ok(prepare_joined_delete_with_options(
        editor,
        reads,
        query,
        query_budget,
        JoinedMutationOptions {
            target_snapshot,
            target_qualifier: target.qualifier,
            mutation: MutationOptions {
                number_format: options.number_format,
                date_format: options.date_format,
                parameters: Some(options.parameters),
                collation: Some(options.collation),
                collation_resolver: Some(options.resolver),
                like_mode: options.like_mode,
                text_collations: &[],
            },
        },
        cancelled,
    )?)
}

#[cfg(test)]
mod tests;
