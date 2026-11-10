//! Shared source-metadata binding and written-order evaluation for INSERT intent.
use super::{InsertError, QueryError, local_insert_table, shape, stored_value};
use crate::{
    ExecutionOptions, MutationOptions,
    binding::{BoundInsertValues, ValuesSource, bind_insert_values},
    discovery::DiscoveryState,
    execution::{Environment, Runtime},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::{InsertValue, InsertableTable};
use tablecraft_expressions::evaluation::{EvalLimits, Evaluator, NoBindings};
use tablecraft_sql::query::{InsertSource, Query};

pub(super) fn prepare<E: InsertableTable>(
    editor: &E,
    query: &Query,
    allow_omissions: bool,
    options: Option<MutationOptions<'_>>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Option<Value>>, InsertError<E::Error>> {
    if options
        .as_ref()
        .is_some_and(|options| !options.text_collations.is_empty())
    {
        return Err(QueryError::Unsupported.into());
    }
    let allow_parameters = options
        .as_ref()
        .is_some_and(|options| options.parameters.is_some());
    local_insert_table(
        query,
        editor.requires_field_list(),
        allow_parameters,
        budget,
        cancelled,
    )?;

    let insert = shape::values(query, allow_parameters)?;
    let InsertSource::Values(expressions) = &insert.source else {
        return Err(QueryError::Unsupported.into());
    };
    let mapping = super::target::mapping(
        editor,
        insert,
        expressions.len(),
        allow_omissions,
        budget,
        cancelled,
    )?;
    let columns = editor.columns();

    budget
        .charge_allocation(columns.len() as u64, size_of::<Option<Value>>() as u64)
        .map_err(QueryError::from)?;
    let mut staged = Vec::new();
    staged
        .try_reserve_exact(columns.len())
        .map_err(|_| QueryError::Allocation)?;
    staged.resize_with(columns.len(), || None);

    if let Some(options) = options {
        // Keep declaration provenance in the query runtime. VALUES never gains
        // input-field identities or a source that can service hidden scans.
        let source = ValuesSource;
        let BoundInsertValues {
            parameters,
            expressions,
        } = bind_insert_values(query, &source, budget, cancelled)?;
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
                collation: options.collation,
                collation_resolver: options.collation_resolver,
                cancelled: Some(cancelled),
                ..Default::default()
            },
            budget,
        };
        runtime.bind_parameters(&parameters, options.parameters.unwrap_or(&[]), true)?;
        let environment = Environment {
            aggregate_results: None,
            collections: None,
            row: &[],
            outer: &[],
            output: &[],
            group: None,
            group_keys: None,
        };
        for (expression, target) in expressions.iter().zip(mapping) {
            check(runtime.budget, cancelled)?;
            let value = runtime.eval(expression, environment)?;
            staged[target] = Some(stored_value(
                value,
                &columns[target],
                runtime.budget,
                cancelled,
            )?);
        }
    } else {
        for (expression, target) in expressions.iter().zip(mapping) {
            check(budget, cancelled)?;
            let value = Evaluator::new(&NoBindings, budget, EvalLimits::default())
                .with_cancellation(cancelled)
                .evaluate(expression)
                .map_err(QueryError::from)?;
            staged[target] = Some(stored_value(value, &columns[target], budget, cancelled)?);
        }
    }

    Ok(staged)
}

pub(super) fn check(
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryError> {
    if cancelled() {
        return Err(QueryError::Cancelled);
    }

    budget.charge(Resource::Steps, 1)?;
    Ok(())
}

pub(super) fn complete(
    staged: Vec<Option<Value>>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<Value>, QueryError> {
    budget.charge_allocation(staged.len() as u64, size_of::<Value>() as u64)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(staged.len())
        .map_err(|_| QueryError::Allocation)?;
    for value in staged {
        check(budget, cancelled)?;
        output.push(value.ok_or(QueryError::Width)?);
    }
    budget.charge(Resource::Rows, 1)?;
    check(budget, cancelled)?;
    Ok(output)
}

pub(super) fn presence(
    staged: Vec<Option<Value>>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<InsertValue>, QueryError> {
    budget.charge_allocation(staged.len() as u64, size_of::<InsertValue>() as u64)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(staged.len())
        .map_err(|_| QueryError::Allocation)?;
    for value in staged {
        check(budget, cancelled)?;
        output.push(value.map_or(InsertValue::Omitted, InsertValue::Supplied));
    }
    budget.charge(Resource::Rows, 1)?;
    check(budget, cancelled)?;
    Ok(output)
}
