use super::{UpdateError, assignment::update_value};
use crate::execution::mutation::{MutationOptions, source::MutationSource};
use crate::{
    ExecutionOptions, QueryError,
    binding::{BoundUpdate, bind_batch_update, bind_update},
    discovery::DiscoveryState,
    execution::{Environment, Runtime},
    source::validate_text_collations,
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_db::{CellUpdate, ColumnUpdates, EditableBatch, EditableTable, UpdateProfile};
use tablecraft_expressions::{comparison::Collation, logic::Truth};
use tablecraft_schema::{NameComparison, ScalarType};
use tablecraft_sql::query::Query;

/// A backend candidate after all matching-row expressions have succeeded.
/// This is not a committed edit; persistence remains an explicit host action.
pub struct UpdatePlan<P> {
    backend: P,
    matched_rows: usize,
}

impl<P> UpdatePlan<P> {
    /// Borrow the validated backend candidate without consuming its query counts.
    pub const fn backend(&self) -> &P {
        &self.backend
    }

    /// Number of WHERE-matching rows, including assignments of unchanged values.
    pub const fn matched_rows(&self) -> usize {
        self.matched_rows
    }

    /// Consume the query wrapper to obtain the backend's validated candidate.
    pub fn into_backend(self) -> P {
        self.backend
    }
}

struct PreparedColumn {
    name: String,
    updates: Vec<CellUpdate>,
}

struct PreparedUpdate {
    columns: Vec<PreparedColumn>,
    matched_rows: usize,
}

/// Plan one local table/one assignment against its frozen original snapshot.
/// ExistingInt32 keeps its old non-NULL checked integral conversion; StoredScalar
/// adds nullable NULL, lossless wider integral assignments and exact other carriers.
/// Complete evaluation precedes one consuming single-column backend finish.
/// No durable write or native authorization is implied by this generic planner.
pub fn plan_update<E: EditableTable>(
    editor: E,
    query: &Query,
    budget: &mut ResourceBudget,
    collation: Option<&dyn Collation>,
    cancelled: &dyn Fn() -> bool,
) -> Result<UpdatePlan<E::Plan>, UpdateError<E::Error>> {
    let _span = tracing::debug_span!("query.update.plan").entered();
    let prepared = prepare(
        &editor,
        query,
        false,
        budget,
        MutationOptions {
            collation,
            ..Default::default()
        },
        cancelled,
    )?;
    let [column] = prepared.columns.as_slice() else {
        return Err(QueryError::Unsupported.into());
    };
    let backend = editor
        .plan_updates(&column.name, &column.updates, budget, cancelled)
        .map_err(UpdateError::Backend)?;
    Ok(UpdatePlan {
        backend,
        matched_rows: prepared.matched_rows,
    })
}

/// Evaluate all SET expressions from ORIGINAL rows before one multi-column finish.
/// Duplicate resolved targets are refused before row evaluation. Each selected row
/// evaluates WHERE once, then every RHS in written order in the unchanged frame.
/// Capability, source constraints and physical publication remain backend-owned.
pub fn plan_batch_update<E: EditableBatch>(
    editor: E,
    query: &Query,
    budget: &mut ResourceBudget,
    collation: Option<&dyn Collation>,
    cancelled: &dyn Fn() -> bool,
) -> Result<UpdatePlan<E::Plan>, UpdateError<E::Error>> {
    let _span = tracing::debug_span!("query.update.batch_plan").entered();
    let prepared = prepare(
        &editor,
        query,
        true,
        budget,
        MutationOptions {
            collation,
            ..Default::default()
        },
        cancelled,
    )?;
    let columns = prepared.borrowed_columns(budget, cancelled)?;
    finish_batch(editor, &prepared, &columns, budget, cancelled)
}

/// Evaluate and allocate the complete original-row request using the query
/// allowance, then finish once using an independent backend allowance.
/// A backend failure never replenishes either pool or retries evaluation.
pub fn plan_batch_update_with_budgets<E: EditableBatch>(
    editor: E,
    query: &Query,
    query_budget: &mut ResourceBudget,
    backend_budget: &mut ResourceBudget,
    collation: Option<&dyn Collation>,
    cancelled: &dyn Fn() -> bool,
) -> Result<UpdatePlan<E::Plan>, UpdateError<E::Error>> {
    let _span = tracing::debug_span!("query.update.batch_plan").entered();
    let prepared = prepare(
        &editor,
        query,
        true,
        query_budget,
        MutationOptions {
            collation,
            ..Default::default()
        },
        cancelled,
    )?;
    let columns = prepared.borrowed_columns(query_budget, cancelled)?;
    finish_batch(editor, &prepared, &columns, backend_budget, cancelled)
}

/// Plan using actual field policies for binding and every original-row cell.
/// Query staging and native finish retain their separate cumulative allowances.
pub fn plan_batch_update_with_options<E: EditableBatch>(
    editor: E,
    query: &Query,
    query_budget: &mut ResourceBudget,
    backend_budget: &mut ResourceBudget,
    comparison: MutationOptions<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<UpdatePlan<E::Plan>, UpdateError<E::Error>> {
    let _span = tracing::debug_span!("query.update.batch_plan").entered();
    let prepared = prepare(&editor, query, true, query_budget, comparison, cancelled)?;
    let columns = prepared.borrowed_columns(query_budget, cancelled)?;
    finish_batch(editor, &prepared, &columns, backend_budget, cancelled)
}

impl PreparedUpdate {
    fn borrowed_columns(
        &self,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<ColumnUpdates<'_>>, QueryError> {
        budget.charge_allocation(
            self.columns.len() as u64,
            size_of::<ColumnUpdates<'_>>() as u64,
        )?;
        let mut columns = Vec::new();
        columns
            .try_reserve_exact(self.columns.len())
            .map_err(|_| QueryError::Allocation)?;
        for column in &self.columns {
            check(budget, cancelled)?;
            columns.push(ColumnUpdates {
                column: &column.name,
                updates: &column.updates,
            });
        }
        check(budget, cancelled)?;
        Ok(columns)
    }
}

fn finish_batch<E: EditableBatch>(
    editor: E,
    prepared: &PreparedUpdate,
    columns: &[ColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<UpdatePlan<E::Plan>, UpdateError<E::Error>> {
    let backend = editor
        .plan_columns(columns, budget, cancelled)
        .map_err(UpdateError::Backend)?;
    Ok(UpdatePlan {
        backend,
        matched_rows: prepared.matched_rows,
    })
}

fn prepare<E: EditableTable>(
    editor: &E,
    query: &Query,
    multiple: bool,
    budget: &mut ResourceBudget,
    comparison: MutationOptions<'_>,
    cancelled: &dyn Fn() -> bool,
) -> Result<PreparedUpdate, QueryError> {
    check(budget, cancelled)?;

    let profile = editor.update_profile();
    if !matches!(
        profile,
        UpdateProfile::ExistingInt32 | UpdateProfile::StoredScalar
    ) || editor.snapshot().policy().names != NameComparison::AsciiInsensitive
    {
        return Err(QueryError::Unsupported);
    }

    let snapshot = editor.snapshot();
    if !comparison.text_collations.is_empty() {
        budget.charge(Resource::Steps, comparison.text_collations.len() as u64)?;
        validate_text_collations(snapshot.columns(), comparison.text_collations)?;
    }
    let source = MutationSource {
        snapshot,
        text_collations: comparison.text_collations,
    };
    let BoundUpdate {
        parameters,
        assignments,
        filter,
    } = if multiple {
        bind_batch_update(
            query,
            &source,
            budget,
            comparison.parameters.is_some(),
            cancelled,
        )?
    } else {
        bind_update(query, &source, budget, cancelled)?
    };

    snapshot
        .rows()
        .len()
        .checked_mul(assignments.len())
        .ok_or(QueryError::StructureLimit)?;
    budget.charge_allocation(assignments.len() as u64, size_of::<PreparedColumn>() as u64)?;
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(assignments.len())
        .map_err(|_| QueryError::Allocation)?;
    for assignment in &assignments {
        check(budget, cancelled)?;
        let target = snapshot
            .columns()
            .get(assignment.column)
            .ok_or(QueryError::UnknownName)?;
        if profile == UpdateProfile::ExistingInt32 && target.data_type() != ScalarType::Int32 {
            return Err(QueryError::Unsupported);
        }
        budget.charge(Resource::Bytes, target.name().len() as u64)?;
        budget.charge(Resource::Steps, target.name().len() as u64)?;
        let mut name = String::new();
        name.try_reserve_exact(target.name().len())
            .map_err(|_| QueryError::Allocation)?;
        name.push_str(target.name());

        columns.push(PreparedColumn {
            name,
            updates: Vec::new(),
        });
    }

    let discovery = DiscoveryState::new(false);
    let mut runtime = Runtime {
        owner: crate::external_sources::SourceNamespace::Root,
        numeric_scratch: Vec::new(),
        repeatable_dynamic: false,
        source: &source,
        external_sources: Vec::new(),
        discovery: &discovery,
        parameters: Vec::new(),
        options: ExecutionOptions {
            collation: comparison.collation,
            collation_resolver: comparison.collation_resolver,
            cancelled: Some(cancelled),
            ..ExecutionOptions::default()
        },
        budget,
    };
    runtime.bind_parameters(&parameters, comparison.parameters.unwrap_or(&[]), true)?;

    let mut matched_rows = 0_usize;
    for (ordinal, row) in snapshot.rows().iter().enumerate() {
        runtime.tick()?;
        runtime.budget.charge(Resource::Rows, 1)?;
        let mut expression_row = runtime.allocate_expression_row(row.len())?;
        for (position, value) in row.iter().enumerate() {
            expression_row.push(source.cell(
                position,
                runtime.copy(value)?,
                comparison.collation_resolver.is_some(),
            ));
        }

        let env = Environment {
            aggregate_results: None,
            collections: None,
            row: &expression_row,
            outer: &[],
            output: &[],
            group: None,
            group_keys: None,
        };
        if let Some(filter) = &filter {
            let selected = runtime.eval(filter, env)?;
            if !Truth::from_value(&selected)?.is_true() {
                continue;
            }
        }

        let handle = snapshot.row_handle(ordinal).ok_or(QueryError::Source)?;
        for (assignment, column) in assignments.iter().zip(&mut columns) {
            runtime.tick()?;
            let target = snapshot
                .columns()
                .get(assignment.column)
                .ok_or(QueryError::UnknownName)?;
            let value = runtime.eval(&assignment.value, env)?;
            let replacement = update_value(value, target, profile, runtime.budget, cancelled)?;

            // Each capacity increase is prepaid, including spare Vec capacity.
            if column.updates.len() == column.updates.capacity() {
                let added = column.updates.capacity().max(1);
                runtime
                    .budget
                    .charge(Resource::Steps, column.updates.len() as u64)?;
                runtime
                    .budget
                    .charge_allocation(added as u64, size_of::<CellUpdate>() as u64)?;
                column
                    .updates
                    .try_reserve_exact(added)
                    .map_err(|_| QueryError::Allocation)?;
            }

            column.updates.push(CellUpdate::new(handle, replacement));
        }

        matched_rows = matched_rows
            .checked_add(1)
            .ok_or(QueryError::StructureLimit)?;
    }
    runtime.tick()?;

    Ok(PreparedUpdate {
        columns,
        matched_rows,
    })
}

fn check(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), QueryError> {
    if cancelled() {
        return Err(QueryError::Cancelled);
    }

    budget.charge(Resource::Steps, 1)?;
    Ok(())
}
