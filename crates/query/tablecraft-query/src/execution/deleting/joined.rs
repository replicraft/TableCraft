//! Complete joined predicates select distinct original handles before native finish.
use super::{DeleteError, DeletePlan, finish};
use crate::{
    ExecutionOptions, JoinedMutationOptions, QueryError, Source,
    binding::{
        JoinedTargetSource, admit_joined_delete, bind_joined_delete, mutation_columns_match,
    },
    discovery::DiscoveryState,
    execution::{
        Environment, Runtime,
        mutation_relations::{self, push},
    },
    source::validate_text_collations,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_db::{DeletableTable, RowHandle};
use tablecraft_expressions::logic::Truth;
use tablecraft_schema::NameComparison;
use tablecraft_sql::query::Query;

/// A fully staged joined DELETE and its actual original editor, without Source borrows.
#[must_use = "finish explicitly or discard the complete joined selection"]
pub struct PreparedJoinedDelete<E: DeletableTable> {
    editor: E,
    selected: Vec<RowHandle>,
}

impl<E: DeletableTable> PreparedJoinedDelete<E> {
    /// Number of distinct original target handles, never joined-frame or cascade count.
    pub fn matched_rows(&self) -> usize {
        self.selected.len()
    }

    /// Consume the complete selection after all read callbacks and pool borrows ended.
    /// Empty selections still invoke the backend's actual admission once.
    ///
    /// # Errors
    /// Propagates cancellation or the backend's own typed source and deletion refusal.
    pub fn finish(
        self,
        backend_budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<DeletePlan<E::Plan>, DeleteError<E::Error>> {
        let _span = tracing::debug_span!("query.delete.joined_finish").entered();
        if cancelled() {
            return Err(QueryError::Cancelled.into());
        }
        finish(self.editor, &self.selected, backend_budget, cancelled)
    }
}

/// Stage one explicit local DISTINCTROW joined target against its frozen snapshot.
/// Target handles come only from that occurrence; other self-join occurrences
/// use the read Source. All ON/WHERE evaluation completes before membership is
/// published. Repeated matches are deduplicated by original identity, never by
/// equal cells. Outer absence cannot invent a target. Native constraints,
/// programs, relationship/cascade policy and persistence remain backend-owned.
///
/// # Errors
/// Refuses missing DISTINCTROW, ambiguous/unsupported targets, source/schema/policy
/// mismatch, invalid parameters, eager expression errors, cancellation and limits
/// before backend finish. No read-provider or alternate planner retry occurs.
pub fn prepare_joined_delete_with_options<E: DeletableTable>(
    editor: E,
    reads: &dyn Source,
    query: &Query,
    query_budget: &mut ResourceBudget,
    options: JoinedMutationOptions<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<PreparedJoinedDelete<E>, DeleteError<E::Error>> {
    let _span = tracing::debug_span!("query.delete.joined_prepare").entered();
    admit_joined_delete(query, options.target_qualifier, query_budget, cancelled)?;
    let snapshot = editor.snapshot();
    if snapshot.id() != options.target_snapshot {
        return Err(QueryError::Source.into());
    }
    if snapshot.policy().names != NameComparison::AsciiInsensitive {
        return Err(QueryError::Unsupported.into());
    }
    query_budget
        .charge(Resource::Steps, snapshot.columns().len() as u64)
        .map_err(QueryError::from)?;
    if !mutation_columns_match(
        reads.columns(snapshot.name())?,
        snapshot.columns(),
        |amount| {
            if cancelled() {
                return Err(QueryError::Cancelled);
            }
            query_budget
                .charge(Resource::Steps, amount as u64)
                .map_err(QueryError::from)?;
            Ok(())
        },
    )? {
        return Err(QueryError::SchemaChanged.into());
    }
    let policies = reads.text_collations(snapshot.name())?;
    validate_text_collations(snapshot.columns(), policies)?;
    if !options.mutation.text_collations.is_empty() {
        query_budget
            .charge(
                Resource::Steps,
                options.mutation.text_collations.len() as u64,
            )
            .map_err(QueryError::from)?;
        if policies != options.mutation.text_collations {
            return Err(QueryError::SchemaChanged.into());
        }
    }
    let bound = bind_joined_delete(
        query,
        reads,
        JoinedTargetSource {
            table: snapshot.name(),
            qualifier: options.target_qualifier,
            columns: snapshot.columns(),
            text_collations: policies,
        },
        query_budget,
        options.mutation.parameters.is_some(),
        cancelled,
    )?;
    let discovery = DiscoveryState::new(false);
    let mut runtime = Runtime {
        owner: crate::external_sources::SourceNamespace::Root,
        numeric_scratch: Vec::new(),
        repeatable_dynamic: false,
        source: reads,
        external_sources: Vec::new(),
        discovery: &discovery,
        parameters: Vec::new(),
        options: ExecutionOptions {
            collation: options.mutation.collation,
            collation_resolver: options.mutation.collation_resolver,
            like_mode: options.mutation.like_mode,
            cancelled: Some(cancelled),
            ..Default::default()
        },
        budget: query_budget,
    };
    runtime.bind_parameters(
        &bound.parameters,
        options.mutation.parameters.unwrap_or(&[]),
        true,
    )?;
    runtime
        .budget
        .charge(Resource::Rows, snapshot.rows().len() as u64)
        .map_err(QueryError::from)?;
    let frames = mutation_relations::frames(
        &mut runtime,
        &bound.relation,
        bound.target.leaf,
        snapshot,
        &mut 0,
    )?;
    runtime
        .budget
        .charge_allocation(snapshot.rows().len() as u64, size_of::<bool>() as u64)
        .map_err(QueryError::from)?;
    let mut membership = Vec::new();
    membership
        .try_reserve_exact(snapshot.rows().len())
        .map_err(|_| QueryError::Allocation)?;
    runtime
        .budget
        .charge(Resource::Steps, snapshot.rows().len() as u64)
        .map_err(QueryError::from)?;
    membership.resize(snapshot.rows().len(), false);
    for frame in frames {
        runtime.tick()?;
        let environment = Environment {
            aggregate_results: None,
            collections: None,
            row: &frame.cells,
            outer: &[],
            output: &[],
            group: None,
            group_keys: None,
        };
        if let Some(filter) = &bound.filter
            && !Truth::from_value(&runtime.eval(filter, environment)?)
                .map_err(QueryError::from)?
                .is_true()
        {
            continue;
        }
        // Evaluate before this membership check, including repeated and absent
        // target frames; a later eager failure invalidates the entire action.
        if let Some(handle) = frame.target {
            let ordinal = snapshot.ordinal(handle).map_err(|_| QueryError::Source)?;
            *membership.get_mut(ordinal).ok_or(QueryError::Source)? = true;
        }
    }
    let mut selected = Vec::new();
    for (ordinal, matched) in membership.into_iter().enumerate() {
        runtime.tick()?;
        if matched {
            let handle = snapshot.row_handle(ordinal).ok_or(QueryError::Source)?;
            push(&mut runtime, &mut selected, handle)?;
        }
    }
    Ok(PreparedJoinedDelete { editor, selected })
}

#[cfg(test)]
mod tests;
