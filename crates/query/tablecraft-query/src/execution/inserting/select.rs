//! Buffer one complete original retrieval before one atomic append preparation.
use super::{InsertError, stored_value, target, values};
use crate::{
    ExecutionOptions, QueryError, Source,
    discovery::execute_staged_source,
    execution::cell::{CellValue, into_scalar},
    profile::SOURCE_SCHEMA_POLICY,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{ExpressionValue, Value},
};
use tablecraft_db::{
    CompleteInsertableBatch, InsertValue, InsertableTable, OmittingInsertableBatch,
};
use tablecraft_expressions::{
    EvalError,
    evaluation::{EvalLimits, Evaluator, NoBindings},
};
use tablecraft_schema::{Column, ScalarType};
use tablecraft_sql::query::{InsertSource, Query, Statement};

/// Complete original SELECT rows paired with their actual consuming batch editor.
/// This is query staging, not native write admission or a durable save capability.
pub struct PreparedInsertSelect<'a, E> {
    editor: E,
    rows: Vec<Vec<Value>>,
    cancelled: Option<&'a dyn Fn() -> bool>,
}

impl<E: CompleteInsertableBatch> PreparedInsertSelect<'_, E> {
    /// Finish once after retrieval releases every source callback/borrow.
    /// Only this phase uses the supplied backend allowance. Failure never retries
    /// evaluation or publishes a partial candidate. Empty rows still reach admission.
    pub fn finish(self, budget: &mut ResourceBudget) -> Result<E::Plan, InsertError<E::Error>> {
        let never_cancelled = || false;
        let cancelled = self.cancelled.unwrap_or(&never_cancelled);
        if cancelled() {
            return Err(QueryError::Cancelled.into());
        }
        self.editor
            .plan_insert_rows(self.rows, budget, cancelled)
            .map_err(InsertError::Backend)
    }
}

/// Complete original-presence SELECT rows paired with their actual batch editor.
/// Missing destination fields remain Omitted; supplied NULL remains a value.
pub struct PreparedOmittingInsertSelect<'a, E> {
    editor: E,
    rows: Vec<Vec<InsertValue>>,
    cancelled: Option<&'a dyn Fn() -> bool>,
}

impl<E: OmittingInsertableBatch> PreparedOmittingInsertSelect<'_, E> {
    /// Consume the whole staged presence batch under one backend allowance.
    /// Native defaults/providers and final admission remain the backend's work.
    pub fn finish(self, budget: &mut ResourceBudget) -> Result<E::Plan, InsertError<E::Error>> {
        let never_cancelled = || false;
        let cancelled = self.cancelled.unwrap_or(&never_cancelled);
        if cancelled() {
            return Err(QueryError::Cancelled.into());
        }
        self.editor
            .plan_insert_rows_with_omissions(self.rows, budget, cancelled)
            .map_err(InsertError::Backend)
    }
}

/// Stage the complete SELECT/UNION result for one local target, without backend work.
/// Retrieval and stored assignment share `query_budget`. The source and target
/// metadata must remain stable throughout. No source is opened or committed here.
/// The returned candidate owns this actual editor and cannot substitute another.
pub fn prepare_insert_select<'a, E: CompleteInsertableBatch>(
    editor: E,
    query: &Query,
    source: &dyn Source,
    query_budget: &mut ResourceBudget,
    parameters: &[(&str, Value)],
    options: ExecutionOptions<'a>,
) -> Result<PreparedInsertSelect<'a, E>, InsertError<E::Error>> {
    let _span = tracing::debug_span!("query.insert.select_prepare").entered();
    let staged = prepare(
        &editor,
        query,
        source,
        query_budget,
        parameters,
        options,
        false,
    )?;
    query_budget
        .charge_allocation(staged.len() as u64, size_of::<Vec<Value>>() as u64)
        .map_err(QueryError::from)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(staged.len())
        .map_err(|_| QueryError::Allocation)?;
    let never_cancelled = || false;
    let cancelled = options.cancelled.unwrap_or(&never_cancelled);
    for row in staged {
        rows.push(values::complete(row, query_budget, cancelled)?);
    }
    values::check(query_budget, cancelled)?;
    Ok(PreparedInsertSelect {
        editor,
        rows,
        cancelled: options.cancelled,
    })
}

/// Stage SELECT rows with original supplied/omitted destination positions.
/// Complete retrieval and assignment success precedes any backend finish. A
/// later failure cannot consume an earlier successful prefix of this relation.
pub fn prepare_insert_select_with_omissions<'a, E: OmittingInsertableBatch>(
    editor: E,
    query: &Query,
    source: &dyn Source,
    query_budget: &mut ResourceBudget,
    parameters: &[(&str, Value)],
    options: ExecutionOptions<'a>,
) -> Result<PreparedOmittingInsertSelect<'a, E>, InsertError<E::Error>> {
    let _span = tracing::debug_span!("query.insert.select_omission_prepare").entered();
    let staged = prepare(
        &editor,
        query,
        source,
        query_budget,
        parameters,
        options,
        true,
    )?;
    query_budget
        .charge_allocation(staged.len() as u64, size_of::<Vec<InsertValue>>() as u64)
        .map_err(QueryError::from)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(staged.len())
        .map_err(|_| QueryError::Allocation)?;
    let never_cancelled = || false;
    let cancelled = options.cancelled.unwrap_or(&never_cancelled);
    for row in staged {
        rows.push(values::presence(row, query_budget, cancelled)?);
    }
    values::check(query_budget, cancelled)?;
    Ok(PreparedOmittingInsertSelect {
        editor,
        rows,
        cancelled: options.cancelled,
    })
}

fn prepare<'a, E: InsertableTable>(
    editor: &E,
    query: &Query,
    source: &'a dyn Source,
    budget: &mut ResourceBudget,
    parameters: &'a [(&str, Value)],
    options: ExecutionOptions<'a>,
    allow_omissions: bool,
) -> Result<Vec<Vec<Option<Value>>>, QueryError> {
    let never_cancelled = || false;
    let cancelled = options.cancelled.unwrap_or(&never_cancelled);
    values::check(budget, cancelled)?;
    let Statement::Insert(insert) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    if !matches!(insert.source, InsertSource::Query(_))
        || insert.external.is_some()
        || (editor.requires_field_list() && insert.fields.is_empty())
        || insert.fields.len() > SOURCE_SCHEMA_POLICY.max_columns
    {
        return Err(QueryError::Unsupported);
    }

    // Destination identities and explicit positions are checked before any
    // retrieval scan, including execution-phase named-schema discovery.
    let input_width = if insert.fields.is_empty() {
        editor.columns().len()
    } else {
        insert.fields.len()
    };
    let mapping = target::mapping(
        editor,
        insert,
        input_width,
        allow_omissions,
        budget,
        cancelled,
    )?;
    let result = execute_staged_source(
        query,
        source,
        parameters,
        options,
        budget,
        Some(mapping.len()),
    )?;

    budget.charge_allocation(
        result.rows.len() as u64,
        size_of::<Vec<Option<Value>>>() as u64,
    )?;
    let mut staged = Vec::new();
    staged
        .try_reserve_exact(result.rows.len())
        .map_err(|_| QueryError::Allocation)?;
    for row in result.rows {
        values::check(budget, cancelled)?;
        if row.len() != mapping.len() {
            return Err(QueryError::Width);
        }
        budget.charge_allocation(
            editor.columns().len() as u64,
            size_of::<Option<Value>>() as u64,
        )?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(editor.columns().len())
            .map_err(|_| QueryError::Allocation)?;
        output.resize_with(editor.columns().len(), || None);
        for (cell, position) in row.into_iter().zip(&mapping) {
            values::check(budget, cancelled)?;
            output[*position] = Some(assign(cell, &editor.columns()[*position], budget, options)?);
        }
        budget.charge(Resource::Rows, 1)?;
        staged.push(output);
    }
    values::check(budget, cancelled)?;
    Ok(staged)
}

fn assign(
    mut cell: CellValue,
    target: &Column,
    budget: &mut ResourceBudget,
    options: ExecutionOptions<'_>,
) -> Result<Value, QueryError> {
    let never_cancelled = || false;
    let cancelled = options.cancelled.unwrap_or(&never_cancelled);
    if matches!(&cell.value, ExpressionValue::StringBytes(_)) {
        if target.data_type() != ScalarType::Text {
            return Err(EvalError::NonScalarExpression.into());
        }
        // Lossless String cells survive the complete read. Unicode publication
        // happens only here, at the explicit stored Text assignment boundary.
        let mut evaluator = Evaluator::new(
            &NoBindings,
            budget,
            EvalLimits {
                max_text_bytes: options.text_limit(),
                ..Default::default()
            },
        )
        .with_cancellation(cancelled);
        cell.value = evaluator.publish_expression_value(cell.value)?;
    }
    stored_value(into_scalar(cell)?, target, budget, cancelled)
}

#[cfg(test)]
mod tests;
