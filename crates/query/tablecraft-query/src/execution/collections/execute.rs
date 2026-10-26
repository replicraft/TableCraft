//! Publication is the only boundary that combines scalar cells and list sidecars.
use super::{CollectionFrame, Runtime};
use crate::{
    CollectionQueryResult, ExecutionOptions, ExpressionQueryResult, OutputType, PreparedQuery,
    QueryError, QueryResult, QueryValue, Source,
    discovery::DiscoveryState,
    execution::{ExecutionContext, execute::ExecutionFrame, result::publish_cells},
    plan::PreparedBody,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

impl PreparedQuery {
    /// Execute with complete stored multivalue lists as structured result cells.
    /// Missing outer-joined parents have explicit `QueryValue::AbsentMultivalue`
    /// results, separate from present empty lists and stored NULL children.
    /// Scalar-only plans retain their ordinary evaluation semantics and limits.
    /// Collection plans admit row-local direct/static named/derived SELECTs
    /// and qualified INNER/LEFT/RIGHT joins or comma products, retaining explicit
    /// absent-parent results. Unsupported collection operations refuse before
    /// any source scan.
    pub fn execute_collections<'a>(
        &self,
        source: &'a dyn Source,
        parameters: &'a [(&str, Value)],
        options: ExecutionOptions<'a>,
    ) -> Result<CollectionQueryResult, QueryError> {
        let mut budget = crate::discovery::budget(options.limits);
        let discovery = DiscoveryState::new(false);
        self.execute_collections_discovered(
            source,
            parameters,
            options,
            &mut budget,
            &discovery,
            ExecutionContext {
                strict_parameters: true,
                repeatable_dynamic: false,
            },
        )
    }

    pub(crate) fn execute_collections_discovered<'a, 'source>(
        &self,
        source: &'source dyn Source,
        parameters: &'a [(&str, Value)],
        options: ExecutionOptions<'a>,
        budget: &'a mut ResourceBudget,
        discovery: &'a DiscoveryState<'source>,
        context: ExecutionContext,
    ) -> Result<CollectionQueryResult, QueryError> {
        let _span = tracing::debug_span!("query.collections.execute").entered();
        let collection_plan = match &self.body {
            PreparedBody::Read(read) if read.has_collections() => Some(read),
            PreparedBody::Crosstab(cross) if cross.read.has_collections() => {
                return Err(QueryError::Unsupported);
            }
            _ => None,
        };
        let Some(plan) = collection_plan else {
            let result =
                self.execute_discovered(source, parameters, options, budget, discovery, context)?;
            let result = publish_cells(result, budget, options)?;
            return wrap_scalar_result(result, budget, options);
        };
        let mut runtime = self.initialize_runtime(
            ExecutionFrame {
                source,
                parameters,
                options,
                discovery,
                context,
            },
            budget,
        )?;
        runtime.check_collection_read(plan, 1)?;
        let frames = runtime.collection_read(plan)?;
        let mut columns = Vec::new();
        for name in &plan.names {
            runtime.tick()?;
            runtime.budget.charge(Resource::Bytes, name.len() as u64)?;
            let mut copied = String::new();
            copied
                .try_reserve_exact(name.len())
                .map_err(|_| QueryError::Allocation)?;
            copied.push_str(name);
            runtime.push(&mut columns, copied)?;
        }
        let types = plan.active_types(options.declared_integral);
        let output_types = runtime.copy_output_types(types)?;
        let mut rows = Vec::new();
        for frame in frames {
            let row = runtime.publish_collection_frame(frame, types)?;
            runtime.push(&mut rows, row)?;
        }
        Ok(QueryResult {
            columns,
            output_types,
            rows,
        })
    }
}

impl Runtime<'_, '_> {
    fn publish_collection_frame(
        &mut self,
        frame: CollectionFrame,
        types: &[OutputType],
    ) -> Result<Vec<QueryValue>, QueryError> {
        self.tick()?;
        self.budget.charge(Resource::Rows, 1)?;
        self.budget
            .charge_allocation(types.len() as u64, size_of::<QueryValue>() as u64)?;
        let mut row = Vec::new();
        row.try_reserve_exact(types.len())
            .map_err(|_| QueryError::Allocation)?;
        let mut scalars = frame.scalars.into_iter();
        let mut lists = frame.lists.into_iter();
        for output in types {
            self.tick()?;
            let value = if output.is_multivalue() {
                let (_, value) = lists.next().ok_or(QueryError::Width)?;
                match value {
                    Some(value) => QueryValue::Multivalue(value),
                    None => QueryValue::AbsentMultivalue,
                }
            } else {
                let cell = scalars.next().ok_or(QueryError::Width)?;
                QueryValue::Scalar(self.evaluator().publish_expression_value(cell.value)?)
            };
            row.push(value);
        }
        if scalars.next().is_some() || lists.next().is_some() {
            return Err(QueryError::Width);
        }
        Ok(row)
    }
}

fn wrap_scalar_result(
    result: ExpressionQueryResult,
    budget: &mut ResourceBudget,
    options: ExecutionOptions<'_>,
) -> Result<CollectionQueryResult, QueryError> {
    let mut rows = Vec::new();
    for row in result.rows {
        if options.cancelled.is_some_and(|cancelled| cancelled()) {
            return Err(QueryError::Cancelled);
        }
        budget.charge(Resource::Steps, 1)?;
        budget.charge_allocation(row.len() as u64, size_of::<QueryValue>() as u64)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(row.len())
            .map_err(|_| QueryError::Allocation)?;
        for value in row {
            if options.cancelled.is_some_and(|cancelled| cancelled()) {
                return Err(QueryError::Cancelled);
            }
            budget.charge(Resource::Steps, 1)?;
            values.push(QueryValue::Scalar(value));
        }
        budget.charge(Resource::Bytes, size_of::<Vec<QueryValue>>() as u64)?;
        rows.try_reserve(1).map_err(|_| QueryError::Allocation)?;
        rows.push(values);
    }
    Ok(QueryResult {
        columns: result.columns,
        output_types: result.output_types,
        rows,
    })
}
