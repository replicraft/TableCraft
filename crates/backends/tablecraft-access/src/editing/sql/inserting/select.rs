//! SELECT staging owns its exact native inserter while source borrows are released.
use super::RecoverableInsertError;
use crate::editing::journal::{
    NativeJournalInsertPlan, NativeJournalInserter, inserting::clock::ClockInserter,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::InsertableTable;
use tablecraft_expressions::functions::dates::clock::Clock;
use tablecraft_query::{
    ExecutionOptions, PreparedInsertSelect, PreparedOmittingInsertSelect, QueryError, Source,
    prepare_insert_select, prepare_insert_select_with_omissions,
};
use tablecraft_sql::query::{InsertSource, Query, Statement};

const MAX_TARGET_NAME_BYTES: usize = 255;
const MAX_TARGET_COLUMNS: usize = 256;

/// Complete original SELECT rows paired with their actual native insertion target.
/// The variants retain complete versus omitted intent and any explicit default
/// clock privately. This is not native admission or permission to save a file.
pub struct PreparedNativeInsertSelect<'a> {
    candidate: Candidate<'a>,
}

enum Candidate<'a> {
    Complete(PreparedInsertSelect<'a, NativeJournalInserter>),
    Omitted(PreparedOmittingInsertSelect<'a, NativeJournalInserter>),
    ClockOmitted(PreparedOmittingInsertSelect<'a, ClockInserter<'a>>),
}

impl PreparedNativeInsertSelect<'_> {
    /// Consume the complete staged relation after every source callback/borrow is
    /// released. Only this step uses the native allowance and selects a sealed
    /// native family; failure never retries retrieval or defaults as another mode.
    pub fn finish(
        self,
        native: &mut ResourceBudget,
    ) -> Result<NativeJournalInsertPlan, RecoverableInsertError> {
        Ok(match self.candidate {
            Candidate::Complete(candidate) => candidate.finish(native)?,
            Candidate::Omitted(candidate) => candidate.finish(native)?,
            Candidate::ClockOmitted(candidate) => candidate.finish(native)?,
        })
    }
}

/// Inspect only an explicit local INSERT SELECT target before any file opens.
/// Source retrieval is left to the caller-supplied Source and existing query
/// services; target paths/providers, VALUES and malformed target widths refuse.
pub fn local_insert_select_table<'a>(
    query: &'a Query,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a str, QueryError> {
    check(budget, cancelled)?;
    let Statement::Insert(insert) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    if insert.external.is_some() || !matches!(insert.source, InsertSource::Query(_)) {
        return Err(QueryError::Unsupported);
    }
    if insert.target.as_str().len() > MAX_TARGET_NAME_BYTES
        || insert.fields.len() > MAX_TARGET_COLUMNS
    {
        return Err(QueryError::StructureLimit);
    }
    for field in &insert.fields {
        check(budget, cancelled)?;
        if field.as_str().len() > MAX_TARGET_NAME_BYTES {
            return Err(QueryError::StructureLimit);
        }
    }
    Ok(insert.target.as_str())
}

/// Buffer a complete original SELECT/UNION result without holding a native
/// mutable allowance across scans. The supplied inserter must already have been
/// captured under that native pool; all retrieval/assignment uses `query_budget`.
/// Missing fields are chosen structurally once from actual target metadata.
pub fn prepare_native_insert_select<'a>(
    inserter: NativeJournalInserter,
    query: &Query,
    source: &dyn Source,
    query_budget: &mut ResourceBudget,
    parameters: &[(&str, Value)],
    options: ExecutionOptions<'a>,
    clock: Option<&'a dyn Clock>,
) -> Result<PreparedNativeInsertSelect<'a>, RecoverableInsertError> {
    let never_cancelled = || false;
    let cancelled = options.cancelled.unwrap_or(&never_cancelled);
    local_insert_select_table(query, query_budget, cancelled)?;
    let Statement::Insert(insert) = &query.statement else {
        return Err(QueryError::Unsupported.into());
    };
    let omitted = !insert.fields.is_empty() && insert.fields.len() < inserter.columns().len();
    let candidate = if omitted {
        match clock {
            Some(provider) => Candidate::ClockOmitted(prepare_insert_select_with_omissions(
                inserter.with_clock(provider),
                query,
                source,
                query_budget,
                parameters,
                options,
            )?),
            None => Candidate::Omitted(prepare_insert_select_with_omissions(
                inserter,
                query,
                source,
                query_budget,
                parameters,
                options,
            )?),
        }
    } else {
        Candidate::Complete(prepare_insert_select(
            inserter,
            query,
            source,
            query_budget,
            parameters,
            options,
        )?)
    };
    Ok(PreparedNativeInsertSelect { candidate })
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
