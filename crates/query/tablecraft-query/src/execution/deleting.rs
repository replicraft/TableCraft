//! All original predicate results precede a single consuming deletion finish.
mod read_source;
use crate::{
    ExecutionOptions, QueryError,
    binding::{BoundDelete, bind_delete},
    discovery::DiscoveryState,
    execution::{
        Environment, Runtime,
        mutation::{MutationOptions, source::MutationSource},
    },
    source::validate_text_collations,
};
pub use read_source::{
    PreparedReadSourceDelete, ReadSourceDeleteOptions, prepare_read_source_delete_with_options,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_db::{DeletableTable, RowHandle};
use tablecraft_expressions::{comparison::Collation, logic::Truth};
use tablecraft_schema::NameComparison;
use tablecraft_sql::query::Query;
use thiserror::Error;

/// Typed predicate/planning failures without SQL, identifiers or row payloads.
#[derive(Debug, Error)]
pub enum DeleteError<E: std::error::Error> {
    /// Binding, expression, allocation, work or cancellation failure.
    #[error(transparent)]
    Query(#[from] QueryError),
    /// Backend rejected the complete deletion selection without durable writes.
    #[error("delete backend rejected the candidate: {0}")]
    Backend(#[source] E),
}

/// One backend deletion candidate after every original-row predicate succeeds.
pub struct DeletePlan<P> {
    backend: P,
    matched_rows: usize,
}

impl<P> DeletePlan<P> {
    /// Original WHERE-matching rows, excluding additional cascade deletions.
    pub const fn matched_rows(&self) -> usize {
        self.matched_rows
    }

    /// Borrow the sealed backend candidate without consuming its query counts.
    pub const fn backend(&self) -> &P {
        &self.backend
    }

    /// Consume the wrapper; persistence remains an explicit host responsibility.
    pub fn into_backend(self) -> P {
        self.backend
    }
}

/// Select original rows of one local table, then prepare one deletion candidate.
/// Pure predicates retain eager errors and explicit collation. Declarations,
/// joins, external sources and hidden retrieval remain closed in this legacy API.
/// Even no matches call the backend once, preserving empty-operation admission.
pub fn plan_delete<E: DeletableTable>(
    editor: E,
    query: &Query,
    budget: &mut ResourceBudget,
    collation: Option<&dyn Collation>,
    cancelled: &dyn Fn() -> bool,
) -> Result<DeletePlan<E::Plan>, DeleteError<E::Error>> {
    let _span = tracing::debug_span!("query.delete.plan").entered();
    let selected = select_rows(
        &editor,
        query,
        budget,
        MutationOptions {
            collation,
            ..Default::default()
        },
        cancelled,
    )?;
    finish(editor, &selected, budget, cancelled)
}

/// Evaluate a complete original-row selection with explicit typed inputs and
/// field comparison policies, then finish with an independent backend allowance.
/// No durable mutation or native authorization is implied by this generic plan.
pub fn plan_delete_with_options<E: DeletableTable>(
    editor: E,
    query: &Query,
    query_budget: &mut ResourceBudget,
    backend_budget: &mut ResourceBudget,
    options: MutationOptions<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<DeletePlan<E::Plan>, DeleteError<E::Error>> {
    let _span = tracing::debug_span!("query.delete.plan").entered();
    let selected = select_rows(&editor, query, query_budget, options, cancelled)?;
    finish(editor, &selected, backend_budget, cancelled)
}

fn finish<E: DeletableTable>(
    editor: E,
    selected: &[RowHandle],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<DeletePlan<E::Plan>, DeleteError<E::Error>> {
    let matched_rows = selected.len();
    let backend = editor
        .plan_deletes(selected, budget, cancelled)
        .map_err(DeleteError::Backend)?;
    Ok(DeletePlan {
        backend,
        matched_rows,
    })
}

fn select_rows<E: DeletableTable>(
    editor: &E,
    query: &Query,
    budget: &mut ResourceBudget,
    options: MutationOptions<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<RowHandle>, QueryError> {
    select_rows_with_source(editor, query, budget, options, None, cancelled)
}

fn select_rows_with_source<E: DeletableTable>(
    editor: &E,
    query: &Query,
    budget: &mut ResourceBudget,
    options: MutationOptions<'_>,
    reads: Option<&dyn crate::Source>,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<RowHandle>, QueryError> {
    if cancelled() {
        return Err(QueryError::Cancelled);
    }

    budget
        .charge(Resource::Steps, 1)
        .map_err(QueryError::from)?;
    let snapshot = editor.snapshot();
    if snapshot.policy().names != NameComparison::AsciiInsensitive {
        return Err(QueryError::Unsupported);
    }

    if !options.text_collations.is_empty() {
        budget.charge(Resource::Steps, options.text_collations.len() as u64)?;
        validate_text_collations(snapshot.columns(), options.text_collations)?;
    }
    let source = MutationSource {
        snapshot,
        text_collations: match reads {
            Some(reads) => reads.text_collations(snapshot.name())?,
            None => options.text_collations,
        },
    };
    validate_text_collations(snapshot.columns(), source.text_collations)?;
    let selected_source = reads.unwrap_or(&source);
    let BoundDelete { parameters, filter } = if reads.is_some() {
        crate::binding::bind_read_source_delete(
            query,
            selected_source,
            snapshot.name(),
            snapshot.columns(),
            options.parameters.is_some(),
            budget,
            cancelled,
        )?
    } else {
        bind_delete(
            query,
            selected_source,
            options.parameters.is_some(),
            budget,
            cancelled,
        )?
    };

    let discovery = DiscoveryState::new(false);
    let mut runtime = Runtime {
        owner: crate::external_sources::SourceNamespace::Root,
        numeric_scratch: Vec::new(),
        repeatable_dynamic: false,
        source: selected_source,
        external_sources: Vec::new(),
        discovery: &discovery,
        parameters: Vec::new(),
        options: ExecutionOptions {
            collation: options.collation,
            collation_resolver: options.collation_resolver,
            cancelled: Some(cancelled),
            ..ExecutionOptions::default()
        },
        budget,
    };
    runtime.tick()?;
    runtime.bind_parameters(&parameters, options.parameters.unwrap_or(&[]), true)?;
    let mut selected: Vec<RowHandle> = Vec::new();

    for (ordinal, row) in snapshot.rows().iter().enumerate() {
        runtime.tick()?;
        runtime
            .budget
            .charge(Resource::Rows, 1)
            .map_err(QueryError::from)?;
        let mut expression_row = runtime.allocate_expression_row(row.len())?;
        for (position, value) in row.iter().enumerate() {
            expression_row.push(source.cell(
                position,
                runtime.copy(value)?,
                options.collation_resolver.is_some(),
            ));
        }

        let environment = Environment {
            aggregate_results: None,
            collections: None,
            row: &expression_row,
            outer: &[],
            output: &[],
            group: None,
            group_keys: None,
        };
        if let Some(filter) = &filter {
            let value = runtime.eval(filter, environment)?;
            if !Truth::from_value(&value)
                .map_err(QueryError::from)?
                .is_true()
            {
                continue;
            }
        }

        let handle = snapshot.row_handle(ordinal).ok_or(QueryError::Source)?;
        if selected.len() == selected.capacity() {
            let added = selected.capacity().max(1);
            runtime
                .budget
                .charge(Resource::Steps, selected.len() as u64)
                .map_err(QueryError::from)?;
            runtime
                .budget
                .charge_allocation(added as u64, size_of::<RowHandle>() as u64)
                .map_err(QueryError::from)?;
            selected
                .try_reserve_exact(added)
                .map_err(|_| QueryError::Allocation)?;
        }

        selected.push(handle);
    }

    runtime.tick()?;
    Ok(selected)
}

#[cfg(test)]
mod tests;
