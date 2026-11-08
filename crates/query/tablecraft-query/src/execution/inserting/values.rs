//! Shared source-metadata binding and written-order evaluation for INSERT intent.
use super::{InsertError, QueryError, local_insert_table, shape, stored_value};
use crate::{
    ExecutionOptions, MutationOptions,
    binding::{BoundInsertValues, ValuesSource, bind_insert_values},
    discovery::DiscoveryState,
    execution::{Environment, Runtime},
    profile::{MAX_BIND_BYTES, SOURCE_SCHEMA_POLICY},
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_db::{InsertValue, InsertableTable};
use tablecraft_expressions::evaluation::{EvalLimits, Evaluator, NoBindings};
use tablecraft_schema::NameComparison;
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
    let target = local_insert_table(
        query,
        editor.requires_field_list(),
        allow_parameters,
        budget,
        cancelled,
    )?;

    let policy = editor.schema_policy();
    let columns = editor.columns();
    if editor.name().len() > SOURCE_SCHEMA_POLICY.max_name_bytes {
        return Err(QueryError::StructureLimit.into());
    }
    if !equal_name(policy.names, target, editor.name(), budget, cancelled)? {
        return Err(QueryError::UnknownName.into());
    }

    if columns.is_empty()
        || columns.len() > SOURCE_SCHEMA_POLICY.max_columns
        || columns.len() > policy.max_columns
    {
        return Err(QueryError::Width.into());
    }

    // Bound and validate every identity before any quadratic comparison work.
    // Column constructors already establish the shared nonempty/control-free law.
    for column in columns {
        check(budget, cancelled)?;
        policy
            .validate_name(column.schema_name())
            .map_err(QueryError::from)?;
        SOURCE_SCHEMA_POLICY
            .validate_name(column.schema_name())
            .map_err(QueryError::from)?;
    }

    for (position, column) in columns.iter().enumerate() {
        for prior in &columns[..position] {
            if equal_name(policy.names, prior.name(), column.name(), budget, cancelled)? {
                return Err(QueryError::AmbiguousName.into());
            }
        }
    }

    let insert = shape::values(query, allow_parameters)?;
    let InsertSource::Values(expressions) = &insert.source else {
        return Err(QueryError::Unsupported.into());
    };
    if expressions.len() > columns.len()
        || ((!allow_omissions || insert.fields.is_empty()) && expressions.len() != columns.len())
    {
        return Err(QueryError::Width.into());
    }

    budget
        .charge_allocation(columns.len() as u64, size_of::<usize>() as u64)
        .map_err(QueryError::from)?;
    let mut mapping = Vec::new();
    mapping
        .try_reserve_exact(columns.len())
        .map_err(|_| QueryError::Allocation)?;
    for position in 0..expressions.len() {
        check(budget, cancelled)?;
        let target = if insert.fields.is_empty() {
            position
        } else {
            let mut found = None;
            for (candidate, column) in columns.iter().enumerate() {
                if equal_name(
                    policy.names,
                    column.name(),
                    insert.fields[position].as_str(),
                    budget,
                    cancelled,
                )? {
                    found = Some(candidate);
                    break;
                }
            }
            found.ok_or(QueryError::UnknownName)?
        };
        budget
            .charge(Resource::Steps, mapping.len() as u64)
            .map_err(QueryError::from)?;
        if mapping.contains(&target) {
            return Err(QueryError::DuplicateInsertField.into());
        }
        mapping.push(target);
    }

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
        budget
            .charge(Resource::Bytes, MAX_BIND_BYTES as u64)
            .map_err(QueryError::from)?;
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

fn equal_name(
    policy: NameComparison,
    left: &str,
    right: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, QueryError> {
    check(budget, cancelled)?;

    // Equality first checks width, then at most this many UTF-8 bytes. Pay before
    // the comparison, including for duplicate schema and reordered field scans.
    budget.charge(Resource::Steps, left.len().min(right.len()) as u64)?;
    Ok(policy.equals(left, right))
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
