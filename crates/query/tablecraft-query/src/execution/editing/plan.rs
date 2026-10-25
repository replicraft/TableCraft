use super::{UpdateError, source::SnapshotSource};
use crate::execution::cell::CellValue;
use crate::{
    ExecutionOptions, QueryError,
    binding::{BoundUpdate, bind_update},
    discovery::DiscoveryState,
    execution::{Environment, Runtime},
    profile::MAX_BIND_BYTES,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::{CellUpdate, EditableTable, UpdateProfile};
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
    /// Number of WHERE-matching rows, including assignments of unchanged values.
    pub const fn matched_rows(&self) -> usize {
        self.matched_rows
    }
    /// Consume the query wrapper to obtain the backend's validated candidate.
    pub fn into_backend(self) -> P {
        self.backend
    }
}

/// Plan one local table/one Int32 assignment UPDATE against a frozen snapshot.
/// Optional alias, pure expressions, BETWEEN/IN and three-valued WHERE use the
/// existing query evaluator. Checked integral narrowing is explicit at assignment.
/// Parameters, joins, subqueries, calls and other actions are rejected before any
/// backend planning. All expressions finish before the single backend call.
/// Caller budgets accumulate across binding allowances/evaluation/backend checks;
/// no QueryLimits budget reset or durable write occurs here.
pub fn plan_update<E: EditableTable>(
    editor: E,
    query: &Query,
    budget: &mut ResourceBudget,
    collation: Option<&dyn Collation>,
    cancelled: &dyn Fn() -> bool,
) -> Result<UpdatePlan<E::Plan>, UpdateError<E::Error>> {
    let _span = tracing::debug_span!("query.update.plan").entered();
    if cancelled() {
        return Err(QueryError::Cancelled.into());
    }
    budget
        .charge(Resource::Steps, 1)
        .map_err(QueryError::from)?;
    if editor.update_profile() != UpdateProfile::ExistingInt32
        || editor.snapshot().policy().names != NameComparison::AsciiInsensitive
    {
        return Err(QueryError::Unsupported.into());
    }
    // Existing binder has independent hard AST/depth/name ceilings. Charge its
    // maximum retained allocation allowance to the shared edit budget beforehand.
    budget
        .charge(Resource::Bytes, MAX_BIND_BYTES as u64)
        .map_err(QueryError::from)?;
    let snapshot = editor.snapshot();
    let source = SnapshotSource(snapshot);
    let BoundUpdate {
        column,
        value,
        filter,
    } = bind_update(query, &source)?;
    let target = snapshot
        .columns()
        .get(column)
        .ok_or(QueryError::UnknownName)?;
    if target.data_type() != ScalarType::Int32 {
        return Err(QueryError::Unsupported.into());
    }
    let mut column_name = String::new();
    budget
        .charge(Resource::Bytes, target.name().len() as u64)
        .map_err(QueryError::from)?;
    column_name
        .try_reserve_exact(target.name().len())
        .map_err(|_| QueryError::Allocation)?;
    column_name.push_str(target.name());
    let discovery = DiscoveryState::new(false);
    let mut runtime = Runtime {
        owner: crate::external_sources::SourceNamespace::Root,
        numeric_scratch: Vec::new(),
        repeatable_dynamic: false,
        domain_depth: 0,
        source: &source,
        external_sources: Vec::new(),
        discovery: &discovery,
        parameters: Vec::new(),
        options: ExecutionOptions {
            collation,
            cancelled: Some(cancelled),
            ..ExecutionOptions::default()
        },
        budget,
    };
    runtime.tick()?;
    let mut updates = Vec::new();
    for (ordinal, row) in snapshot.rows().iter().enumerate() {
        runtime.tick()?;
        runtime
            .budget
            .charge(Resource::Rows, 1)
            .map_err(QueryError::from)?;
        let mut expression_row = runtime.allocate_expression_row(row.len())?;
        for value in row {
            expression_row.push(CellValue::scalar(runtime.copy(value)?));
        }
        let env = Environment {
            collections: None,
            row: &expression_row,
            outer: &[],
            output: &[],
            group: None,
            group_keys: None,
        };
        if let Some(filter) = &filter {
            let selected = runtime.eval(filter, env)?;
            if !Truth::from_value(&selected)
                .map_err(QueryError::from)?
                .is_true()
            {
                continue;
            }
        }
        let replacement = integral_assignment(runtime.eval(&value, env)?)?;
        let handle = snapshot.row_handle(ordinal).ok_or(QueryError::Source)?;
        runtime.push(&mut updates, CellUpdate::new(handle, replacement))?;
    }
    runtime.tick()?;
    let matched_rows = updates.len();
    let backend = editor
        .plan_updates(&column_name, &updates, budget, cancelled)
        .map_err(UpdateError::Backend)?;
    Ok(UpdatePlan {
        backend,
        matched_rows,
    })
}

fn integral_assignment(value: Value) -> Result<Value, QueryError> {
    let integer = match value {
        Value::Byte(value) => i64::from(value),
        Value::Int16(value) => i64::from(value),
        Value::Int32(value) => i64::from(value),
        Value::Int64(value) => value,
        _ => return Err(QueryError::Unsupported),
    };
    i32::try_from(integer)
        .map(Value::Int32)
        .map_err(|_| tablecraft_expressions::EvalError::Overflow.into())
}
