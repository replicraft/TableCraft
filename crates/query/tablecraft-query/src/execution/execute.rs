use super::cell::{CellQueryResult, into_scalar};
use super::{ExecutionOptions, ExpressionQueryResult, QueryResult, Runtime};
use crate::discovery::DiscoveryState;
use crate::{PreparedQuery, QueryError, Source, plan::PreparedBody};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

impl PreparedQuery {
    /// Execute against explicit sources and exactly the declared parameter names.
    /// Parameters must already have their exact declared carriers; NULL is allowed.
    /// A source must keep its schema/data snapshot stable throughout this call.
    pub fn execute<'a>(
        &self,
        source: &'a dyn Source,
        parameters: &'a [(&str, Value)],
        options: ExecutionOptions<'a>,
    ) -> Result<QueryResult, QueryError> {
        self.execute_with(source, parameters, options, |runtime, result| {
            let mut rows = Vec::new();
            for row in result.rows {
                let mut values = runtime.allocate_row(row.len())?;
                for value in row {
                    runtime.tick()?;
                    values.push(into_scalar(value)?);
                }
                runtime.push(&mut rows, values)?;
            }
            Ok(QueryResult {
                columns: result.columns,
                output_types: result.output_types,
                rows,
            })
        })
    }

    /// Execute with explicit expression states in result and intermediate rows.
    /// This never changes scalar source, parameter or database write contracts.
    pub fn execute_expression<'a>(
        &self,
        source: &'a dyn Source,
        parameters: &'a [(&str, Value)],
        options: ExecutionOptions<'a>,
    ) -> Result<ExpressionQueryResult, QueryError> {
        self.execute_with(source, parameters, options, |runtime, result| {
            runtime.publish_cells(result)
        })
    }

    fn execute_with<'a, T>(
        &self,
        source: &'a dyn Source,
        parameters: &'a [(&str, Value)],
        options: ExecutionOptions<'a>,
        publish: impl FnOnce(
            &mut Runtime<'_, '_>,
            CellQueryResult,
        ) -> Result<QueryResult<T>, QueryError>,
    ) -> Result<QueryResult<T>, QueryError> {
        let mut budget = crate::discovery::budget(options.limits);
        let discovery = DiscoveryState::new(false);
        self.execute_budgeted(
            ExecutionFrame {
                source,
                parameters,
                options,
                discovery: &discovery,
                strict_parameters: true,
            },
            &mut budget,
            publish,
        )
    }

    pub(crate) fn execute_discovered<'a, 'source>(
        &self,
        source: &'source dyn Source,
        parameters: &'a [(&str, Value)],
        options: ExecutionOptions<'a>,
        budget: &'a mut ResourceBudget,
        discovery: &'a DiscoveryState<'source>,
        strict_parameters: bool,
    ) -> Result<CellQueryResult, QueryError> {
        self.execute_budgeted(
            ExecutionFrame {
                source,
                parameters,
                options,
                discovery,
                strict_parameters,
            },
            budget,
            |_, result| Ok(result),
        )
    }

    fn execute_budgeted<'a, 'source, T>(
        &self,
        frame: ExecutionFrame<'a, 'source>,
        budget: &'a mut ResourceBudget,
        publish: impl FnOnce(
            &mut Runtime<'_, '_>,
            CellQueryResult,
        ) -> Result<QueryResult<T>, QueryError>,
    ) -> Result<QueryResult<T>, QueryError> {
        let ExecutionFrame {
            source,
            parameters,
            options,
            discovery,
            strict_parameters,
        } = frame;
        let _span = tracing::debug_span!("query.execute", parameters = parameters.len()).entered();
        let mut runtime = Runtime {
            mean_scratch: Vec::new(),
            repeatable_dynamic: false,
            domain_depth: 0,
            source,
            discovery,
            parameters: Vec::new(),
            options,
            budget,
        };
        runtime.tick()?;
        if strict_parameters && parameters.len() != self.parameters.len() {
            return Err(QueryError::Parameter);
        }
        runtime.budget.charge_allocation(
            self.parameters.len() as u64,
            std::mem::size_of::<&Value>() as u64,
        )?;
        runtime
            .parameters
            .try_reserve_exact(self.parameters.len())
            .map_err(|_| QueryError::Allocation)?;
        for declared in &self.parameters {
            let mut matches = parameters.iter().filter(|(name, _)| {
                crate::profile::SOURCE_SCHEMA_POLICY
                    .names
                    .equals(name, declared.name.as_str())
            });
            let (_, value) = matches.next().ok_or(QueryError::Parameter)?;
            if matches.next().is_some()
                || declared.data_type.validate_value(value).is_err()
                || matches!(value,Value::Text(t) if t.len()>options.text_limit())
                || matches!(value,Value::Single(v) if !v.to_float().is_finite())
                || matches!(value,Value::Double(v) if !v.to_float().is_finite())
            {
                return Err(QueryError::Parameter);
            }
            runtime.parameters.push(value);
        }
        let (rows, names, types) = match &self.body {
            PreparedBody::Read(read) => (
                runtime.read(read, &[])?,
                read.names.as_slice(),
                read.output_types.as_slice(),
            ),
            PreparedBody::Crosstab(cross) => {
                let result = runtime.crosstab(cross)?;
                runtime.tick()?;
                let result = publish(&mut runtime, result)?;
                tracing::debug!(
                    rows = result.rows.len(),
                    columns = result.columns.len(),
                    "query completed"
                );
                return Ok(result);
            }
        };
        let mut columns = Vec::new();
        for name in names {
            runtime.budget.charge(Resource::Bytes, name.len() as u64)?;
            let mut owned = String::new();
            owned
                .try_reserve_exact(name.len())
                .map_err(|_| QueryError::Allocation)?;
            owned.push_str(name);
            runtime.push(&mut columns, owned)?;
        }
        runtime.tick()?;
        let output_types = runtime.copy_output_types(types)?;
        let result = publish(
            &mut runtime,
            QueryResult {
                columns,
                output_types,
                rows,
            },
        )?;
        tracing::debug!(
            rows = result.rows.len(),
            columns = result.columns.len(),
            "query completed"
        );
        Ok(result)
    }
}

struct ExecutionFrame<'a, 'source> {
    source: &'source dyn Source,
    parameters: &'a [(&'a str, Value)],
    options: ExecutionOptions<'a>,
    discovery: &'a DiscoveryState<'source>,
    strict_parameters: bool,
}
