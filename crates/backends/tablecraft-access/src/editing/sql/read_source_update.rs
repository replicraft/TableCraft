//! Explicit predicate mode is selected before any consuming native UPDATE plan.
use super::{NativeMutationOptions, RecoverableError, local_batch_update_table, predicate_reads};
use crate::editing::journal::NativeJournalEditor;
use tablecraft_core::limits::ResourceBudget;
use tablecraft_db::EditableTable;
use tablecraft_query::{
    MutationOptions, PreparedReadSourceUpdate, QueryError, ReadSourceUpdateOptions, Source,
    prepare_read_source_batch_update_with_options,
};
use tablecraft_sql::query::{Query, Statement};

/// Identify written predicate/RHS subqueries before native resource capture.
/// Unsupported retrieval forms still enter explicit preparation and refuse there;
/// a failed binder or native plan is never retried as the legacy action.
/// This metadata-free walk opens no source and grants no mutation authority.
///
/// # Errors
/// Refuses nonlocal/non-UPDATE syntax, cancellation, allocation or caller limits.
pub fn update_uses_read_source(
    query: &Query,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, QueryError> {
    predicate_reads::check(budget, cancelled)?;
    local_batch_update_table(query, true)?;
    let Statement::Update(update) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    predicate_reads::uses_reads(
        update.selection.iter().chain(
            update
                .assignments
                .iter()
                .map(|assignment| &assignment.value),
        ),
        budget,
        cancelled,
    )
}

/// Stage original-row predicates without retaining a native resource-pool borrow.
/// The existing native editor and original intent still qualify actual writes;
/// this service neither opens another source nor widens writer eligibility.
///
/// # Errors
/// Retains typed shape/source/query failures. A mismatched native target is refused
/// before scans, and backend qualification remains the consuming finish's job.
pub fn prepare_native_read_source_update(
    editor: NativeJournalEditor,
    query: &Query,
    reads: &dyn Source,
    query_budget: &mut ResourceBudget,
    options: NativeMutationOptions<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<PreparedReadSourceUpdate<NativeJournalEditor>, RecoverableError> {
    let _span = tracing::debug_span!("access.update.read_source_prepare").entered();
    let table = local_batch_update_table(query, true)?;
    if !editor.snapshot().name().eq_ignore_ascii_case(table) {
        return Err(QueryError::Source.into());
    }
    let target_snapshot = editor.snapshot().id();
    Ok(prepare_read_source_batch_update_with_options(
        editor,
        reads,
        query,
        query_budget,
        ReadSourceUpdateOptions {
            target_snapshot,
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
