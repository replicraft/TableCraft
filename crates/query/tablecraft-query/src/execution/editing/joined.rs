//! Explicit read-source joins never replace the editor's original target identity.

use super::{
    UpdateError, UpdatePlan,
    assignment::update_value,
    plan::{PreparedColumn, PreparedUpdate, finish_batch},
};
use crate::{
    ExecutionOptions, MutationOptions, QueryError, Source,
    binding::{JoinedTargetSource, bind_joined_update},
    discovery::DiscoveryState,
    execution::{
        Environment, Runtime,
        mutation_relations::{self as relation, push},
    },
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::{CellUpdate, EditableBatch, SnapshotId, UpdateProfile};
use tablecraft_expressions::logic::Truth;
use tablecraft_schema::{NameComparison, ScalarType};
use tablecraft_sql::query::Query;

/// Explicit target occurrence and frozen snapshot precondition for one joined action.
/// Read-source schemas/rows retain the ordinary Source stability contract; this
/// does not invent a revision identity for the independently supplied read Source.
pub struct JoinedMutationOptions<'a> {
    /// Exact editor snapshot, including an empty original target.
    pub target_snapshot: SnapshotId,
    /// Written table qualifier or alias identifying exactly one target occurrence.
    pub target_qualifier: &'a str,
    /// Existing typed parameters and comparison policies for this operation.
    pub mutation: MutationOptions<'a>,
}

/// Complete joined evaluation plus its original editor, ready for one consuming
/// backend finish. Preparation retains none of its read-source, comparison-
/// provider or budget borrows.
#[must_use = "finish explicitly or discard the staged joined update"]
pub struct PreparedJoinedUpdate<E: EditableBatch> {
    editor: E,
    prepared: PreparedUpdate,
}

impl<E: EditableBatch> PreparedJoinedUpdate<E> {
    /// Number of distinct selected original target rows, including no-op values.
    pub const fn matched_rows(&self) -> usize {
        self.prepared.matched_rows
    }

    /// Finish once after read-source callbacks and their resource borrows ended.
    /// Borrowed column-view allocation and backend qualification spend this pool;
    /// no SQL expression or read-source callback is evaluated again.
    ///
    /// # Errors
    /// Cancellation, view-allocation or work exhaustion retains QueryError;
    /// backend admission failures retain their original typed backend cause.
    pub fn finish(
        self,
        backend_budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<UpdatePlan<E::Plan>, UpdateError<E::Error>> {
        let _span = tracing::debug_span!("query.update.joined_finish").entered();
        if cancelled() {
            return Err(QueryError::Cancelled.into());
        }
        let columns = self.prepared.borrowed_columns(backend_budget, cancelled)?;
        finish_batch(
            self.editor,
            &self.prepared,
            &columns,
            backend_budget,
            cancelled,
        )
    }
}

struct JoinedColumn {
    name: String,
    values: Vec<Option<Value>>,
}

/// Evaluate scalar local joins and all selected SET expressions before one
/// staged action. No backend budget is borrowed during read-source scans.
/// Selected target rows always come from the editor's
/// original snapshot, never from equality matching or read-source row ordering.
/// INNER/LEFT/RIGHT and comma products are supported. Derived/collection sources,
/// assignments spanning target occurrences and repeated selected target matches
/// remain explicitly unsupported. Native write admission remains backend-owned.
///
/// # Errors
/// Refuses mismatched snapshot/schema preconditions, ambiguous or unsupported
/// targets, expression errors, cancellation, allocation or budget failures before
/// backend finish. Backend qualification failures retain their own typed cause.
pub fn prepare_joined_batch_update_with_options<E: EditableBatch>(
    editor: E,
    reads: &dyn Source,
    query: &Query,
    query_budget: &mut ResourceBudget,
    options: JoinedMutationOptions<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<PreparedJoinedUpdate<E>, UpdateError<E::Error>> {
    let _span = tracing::debug_span!("query.update.joined_prepare").entered();
    if cancelled() {
        return Err(QueryError::Cancelled.into());
    }
    query_budget
        .charge(Resource::Steps, 1)
        .map_err(QueryError::from)?;
    let snapshot = editor.snapshot();
    if snapshot.id() != options.target_snapshot {
        return Err(QueryError::Source.into());
    }
    let profile = editor.update_profile();
    if snapshot.policy().names != NameComparison::AsciiInsensitive
        || !matches!(
            profile,
            UpdateProfile::ExistingInt32 | UpdateProfile::StoredScalar
        )
    {
        return Err(QueryError::Unsupported.into());
    }
    query_budget
        .charge(Resource::Steps, snapshot.columns().len() as u64)
        .map_err(QueryError::from)?;
    if reads.columns(snapshot.name())? != snapshot.columns() {
        return Err(QueryError::SchemaChanged.into());
    }
    if !options.mutation.text_collations.is_empty()
        && reads.text_collations(snapshot.name())? != options.mutation.text_collations
    {
        return Err(QueryError::SchemaChanged.into());
    }
    let bound = bind_joined_update(
        query,
        reads,
        JoinedTargetSource {
            table: snapshot.name(),
            qualifier: options.target_qualifier,
            columns: snapshot.columns(),
            text_collations: reads.text_collations(snapshot.name())?,
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
    let frames = relation::frames(
        &mut runtime,
        &bound.relation,
        bound.target.leaf,
        snapshot,
        &mut 0,
    )?;
    let mut columns = Vec::new();
    for assignment in &bound.assignments {
        runtime.tick()?;
        let column = snapshot
            .columns()
            .get(assignment.column)
            .ok_or(QueryError::Source)?;
        if profile == UpdateProfile::ExistingInt32 && column.data_type() != ScalarType::Int32 {
            return Err(QueryError::Unsupported.into());
        }
        runtime
            .budget
            .charge_allocation(column.name().len() as u64, 1)
            .map_err(QueryError::from)?;
        let mut name = String::new();
        name.try_reserve_exact(column.name().len())
            .map_err(|_| QueryError::Allocation)?;
        name.push_str(column.name());
        runtime
            .budget
            .charge_allocation(
                snapshot.rows().len() as u64,
                size_of::<Option<Value>>() as u64,
            )
            .map_err(QueryError::from)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(snapshot.rows().len())
            .map_err(|_| QueryError::Allocation)?;
        for _ in snapshot.rows() {
            runtime.tick()?;
            values.push(None);
        }
        push(&mut runtime, &mut columns, JoinedColumn { name, values })?;
    }
    runtime
        .budget
        .charge_allocation(snapshot.rows().len() as u64, size_of::<bool>() as u64)
        .map_err(QueryError::from)?;
    let mut selected = Vec::new();
    selected
        .try_reserve_exact(snapshot.rows().len())
        .map_err(|_| QueryError::Allocation)?;
    selected.resize(snapshot.rows().len(), false);
    let mut ambiguous = false;
    let mut matched_rows = 0_usize;
    for frame in frames {
        runtime.tick()?;
        let env = Environment {
            aggregate_results: None,
            collections: None,
            row: &frame.cells,
            outer: &[],
            output: &[],
            group: None,
            group_keys: None,
        };
        if let Some(filter) = &bound.filter
            && !Truth::from_value(&runtime.eval(filter, env)?)
                .map_err(QueryError::from)?
                .is_true()
        {
            continue;
        }
        let Some(handle) = frame.target else {
            continue;
        };
        let ordinal = snapshot.ordinal(handle).map_err(|_| QueryError::Source)?;
        let duplicate = selected[ordinal];
        ambiguous |= duplicate;
        for (assignment, column) in bound.assignments.iter().zip(&mut columns) {
            let value = runtime.eval(&assignment.value, env)?;
            let target = &snapshot.columns()[assignment.column];
            let replacement = update_value(value, target, profile, runtime.budget, cancelled)?;
            if !duplicate {
                column.values[ordinal] = Some(replacement);
            }
        }
        if !duplicate {
            selected[ordinal] = true;
            matched_rows = matched_rows
                .checked_add(1)
                .ok_or(QueryError::StructureLimit)?;
        }
    }
    if ambiguous {
        return Err(QueryError::Unsupported.into());
    }
    runtime.tick()?;
    // Backend requests retain original target order independently of join-side
    // row order. Values move out of their paid staging slots without copying.
    let mut prepared_columns = Vec::new();
    for column in columns {
        let mut updates = Vec::new();
        for (ordinal, replacement) in column.values.into_iter().enumerate() {
            runtime.tick()?;
            if let Some(replacement) = replacement {
                let handle = snapshot.row_handle(ordinal).ok_or(QueryError::Source)?;
                push(
                    &mut runtime,
                    &mut updates,
                    CellUpdate::new(handle, replacement),
                )?;
            }
        }
        push(
            &mut runtime,
            &mut prepared_columns,
            PreparedColumn {
                name: column.name,
                updates,
            },
        )?;
    }
    let prepared = PreparedUpdate {
        columns: prepared_columns,
        matched_rows,
    };
    Ok(PreparedJoinedUpdate { editor, prepared })
}

/// Prepare and finish a joined UPDATE when the read Source does not need to
/// borrow the supplied backend pool during scans. For a shared native pool,
/// prepare first, then borrow it only for PreparedJoinedUpdate::finish.
///
/// # Errors
/// Returns the same typed preparation and finish failures as the two-phase API.
/// No retry, duplicated evaluation or backend finish occurs after a failure.
pub fn plan_joined_batch_update_with_options<E: EditableBatch>(
    editor: E,
    reads: &dyn Source,
    query: &Query,
    query_budget: &mut ResourceBudget,
    backend_budget: &mut ResourceBudget,
    options: JoinedMutationOptions<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<UpdatePlan<E::Plan>, UpdateError<E::Error>> {
    prepare_joined_batch_update_with_options(
        editor,
        reads,
        query,
        query_budget,
        options,
        cancelled,
    )?
    .finish(backend_budget, cancelled)
}

#[cfg(test)]
mod tests;
