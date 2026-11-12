//! Qualified target syntax and staged joined UPDATE over an existing native editor.
use super::{NativeMutationOptions, RecoverableError, joined_target};
use crate::editing::journal::NativeJournalEditor;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_db::EditableTable;
use tablecraft_query::{
    JoinedMutationOptions, MutationOptions, PreparedJoinedUpdate, QueryError, Source,
    prepare_joined_batch_update_with_options,
};
use tablecraft_sql::{
    query::{Query, Statement},
    select::TableSource,
};

/// One written local table occurrence, not native source or write authority.
#[derive(Clone, Copy)]
pub struct JoinedUpdateTarget<'a> {
    /// Actual written source table name, never the alias used as native identity.
    pub table: &'a str,
    /// Exact written qualifier identifying one relation occurrence.
    pub qualifier: &'a str,
}

/// Resolve qualified SET targets to exactly one local joined table occurrence.
/// This is syntax-only; it opens no source, acquires no lease and grants no
/// native eligibility. Unqualified or multiple-occurrence SET targets remain
/// outside this host entrypoint even when a metadata-aware binder could resolve them.
///
/// # Errors
/// Refuses unsupported statement/source shapes, absent or ambiguous qualifiers,
/// cancellation, allocation failure and explicit work exhaustion.
pub fn local_joined_update_target<'a>(
    query: &'a Query,
    allow_parameters: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<JoinedUpdateTarget<'a>, QueryError> {
    check(budget, cancelled)?;
    let Statement::Update(update) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    if !allow_parameters && !query.parameters.is_empty()
        || update.assignments.is_empty()
        || update.sources.is_empty()
        || matches!(update.sources.as_slice(), [TableSource::Table(_)])
    {
        return Err(QueryError::Unsupported);
    }
    let qualifier = update.assignments[0]
        .target
        .qualifier
        .as_ref()
        .ok_or(QueryError::Unsupported)?
        .as_str();
    for assignment in &update.assignments {
        check(budget, cancelled)?;
        let name = assignment
            .target
            .qualifier
            .as_ref()
            .ok_or(QueryError::Unsupported)?
            .as_str();
        budget.charge(Resource::Steps, name.len().max(qualifier.len()) as u64)?;
        if !name.eq_ignore_ascii_case(qualifier) {
            return Err(QueryError::Unsupported);
        }
    }
    let selected = joined_target::find(&update.sources, qualifier, budget, cancelled)?;
    Ok(JoinedUpdateTarget {
        table: selected.table.name.as_str(),
        qualifier: selected
            .table
            .alias
            .as_ref()
            .unwrap_or(&selected.table.name)
            .as_str(),
    })
}

/// Stage all joined expressions after the caller released the native budget
/// borrow used to capture the editor. Source callbacks retain their existing
/// native and comparison pools; only query staging consumes `query_budget`.
/// Finish the returned owner only after its source callbacks have ended.
///
/// # Errors
/// Refuses unsupported target syntax, mismatched editor identity, query/source
/// errors, cancellation, allocation failure and explicit budget exhaustion.
pub fn prepare_native_joined_update(
    editor: NativeJournalEditor,
    query: &Query,
    reads: &dyn Source,
    query_budget: &mut ResourceBudget,
    options: NativeMutationOptions<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<PreparedJoinedUpdate<NativeJournalEditor>, RecoverableError> {
    let _span = tracing::debug_span!("access.joined_update.prepare").entered();
    let target = local_joined_update_target(query, true, query_budget, cancelled)?;
    if !editor.snapshot().name().eq_ignore_ascii_case(target.table) {
        return Err(QueryError::Source.into());
    }
    let target_snapshot = editor.snapshot().id();
    Ok(prepare_joined_batch_update_with_options(
        editor,
        reads,
        query,
        query_budget,
        JoinedMutationOptions {
            target_snapshot,
            target_qualifier: target.qualifier,
            mutation: MutationOptions {
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

fn check(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), QueryError> {
    if cancelled() {
        return Err(QueryError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}

#[cfg(test)]
mod tests;
