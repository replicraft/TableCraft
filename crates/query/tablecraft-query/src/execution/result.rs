use super::{ExecutionOptions, Runtime, cell::CellQueryResult};
use crate::{OutputType, QueryError};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::{ExpressionValue, Value},
};
use tablecraft_expressions::evaluation::{EvalLimits, Evaluator, NoBindings};

/// Fully buffered result, with no Debug/Serde payload exposure.
pub struct QueryResult<T = Value> {
    pub(super) columns: Vec<String>,
    pub(super) output_types: Vec<OutputType>,
    pub(super) rows: Vec<Vec<T>>,
}
/// Query data that may include non-storable expression states.
pub type ExpressionQueryResult = QueryResult<ExpressionValue>;

impl<T> QueryResult<T> {
    /// Ordered output names, preserving aliases and possible duplicates.
    pub fn columns(&self) -> &[String] {
        &self.columns
    }
    /// Conservative scalar/collection metadata aligned with output names. Dynamic entries
    /// retain unknown/varying carriers; no type is inferred from returned rows.
    /// Metadata uses this execution's selected arithmetic profile, which can be
    /// more specific than a prepared query's combined profile description.
    pub fn output_types(&self) -> &[OutputType] {
        &self.output_types
    }
    /// Materialized rows; no storage cursor/updateability is implied.
    pub fn rows(&self) -> &[Vec<T>] {
        &self.rows
    }
}

impl Runtime<'_, '_> {
    pub(super) fn copy_output_types(
        &mut self,
        types: &[OutputType],
    ) -> Result<Vec<OutputType>, QueryError> {
        self.budget
            .charge_allocation(types.len() as u64, size_of::<OutputType>() as u64)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(types.len())
            .map_err(|_| QueryError::Allocation)?;
        result.extend_from_slice(types);
        Ok(result)
    }
}

impl Runtime<'_, '_> {
    pub(super) fn publish_cells(
        &mut self,
        result: CellQueryResult,
    ) -> Result<ExpressionQueryResult, QueryError> {
        publish_cells(result, self.budget, self.options)
    }
}

pub(crate) fn publish_cells(
    result: CellQueryResult,
    budget: &mut ResourceBudget,
    options: ExecutionOptions<'_>,
) -> Result<ExpressionQueryResult, QueryError> {
    let mut rows = Vec::new();
    for row in result.rows {
        if options.cancelled.is_some_and(|callback| callback()) {
            return Err(QueryError::Cancelled);
        }
        budget.charge(Resource::Steps, 1)?;
        budget.charge_allocation(row.len() as u64, size_of::<ExpressionValue>() as u64)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(row.len())
            .map_err(|_| QueryError::Allocation)?;
        for cell in row {
            if options.cancelled.is_some_and(|callback| callback()) {
                return Err(QueryError::Cancelled);
            }
            budget.charge(Resource::Steps, 1)?;
            let value = if matches!(&cell.value, ExpressionValue::StringBytes(_)) {
                let mut evaluator = Evaluator::new(
                    &NoBindings,
                    budget,
                    EvalLimits {
                        max_text_bytes: options.text_limit(),
                        ..Default::default()
                    },
                );
                if let Some(cancelled) = options.cancelled {
                    evaluator = evaluator.with_cancellation(cancelled);
                }
                evaluator.publish_expression_value(cell.value)?
            } else {
                cell.value
            };
            values.push(value);
        }
        budget.charge(Resource::Bytes, size_of::<Vec<ExpressionValue>>() as u64)?;
        rows.try_reserve(1).map_err(|_| QueryError::Allocation)?;
        rows.push(values);
    }
    Ok(QueryResult {
        columns: result.columns,
        output_types: result.output_types,
        rows,
    })
}
