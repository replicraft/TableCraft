use super::cell::{CellQueryResult, into_scalar};
use super::context::ParameterValue;
use super::{ExecutionOptions, ExpressionQueryResult, QueryResult, Runtime};
use crate::discovery::DiscoveryState;
use crate::{PreparedQuery, QueryError, Source, plan::PreparedBody};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

impl PreparedQuery {
    /// Execute against explicit sources and exactly the declared parameter names.
    /// Scalar parameters require exact carriers; VALUE retains the supplied scalar
    /// carrier. NULL is allowed. Neither form performs implicit input coercion.
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
                    let mut value = value;
                    value.value = runtime.evaluator().publish_expression_value(value.value)?;
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
    /// Lossless Strings survive intermediates; final Strings require Unicode
    /// publication under the operation's text/work limits in both result modes.
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
                context: ExecutionContext {
                    strict_parameters: true,
                    repeatable_dynamic: false,
                },
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
        context: ExecutionContext,
    ) -> Result<CellQueryResult, QueryError> {
        self.execute_budgeted(
            ExecutionFrame {
                source,
                parameters,
                options,
                discovery,
                context,
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
            context,
        } = frame;
        let _span = tracing::debug_span!("query.execute", parameters = parameters.len()).entered();
        let mut runtime = Runtime {
            owner: crate::external_sources::SourceNamespace::Root,
            numeric_scratch: Vec::new(),
            repeatable_dynamic: context.repeatable_dynamic,
            domain_depth: 0,
            source,
            external_sources: Vec::new(),
            discovery,
            parameters: Vec::new(),
            options,
            budget,
        };
        runtime.tick()?;
        if context.strict_parameters && parameters.len() != self.parameters.len() {
            return Err(QueryError::Parameter);
        }
        runtime.budget.charge_allocation(
            self.parameters.len() as u64,
            std::mem::size_of::<ParameterValue<'_>>() as u64,
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
                || !declared.data_type.accepts(value)
                || matches!(value, Value::Text(text) if text.len() > options.text_limit())
                || matches!(value, Value::Single(value) if !value.to_float().is_finite())
                || matches!(value, Value::Double(value) if !value.to_float().is_finite())
            {
                return Err(QueryError::Parameter);
            }
            runtime.parameters.push(ParameterValue {
                value,
                declaration: declared.data_type.declaration(),
            });
        }
        runtime.register_external_sources(&self.external_sources)?;
        let (rows, names, types) = match &self.body {
            PreparedBody::Read(read) => (
                runtime.read(read, &[])?,
                read.names.as_slice(),
                read.active_types(options.declared_integral),
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
    context: ExecutionContext,
}

pub(crate) struct ExecutionContext {
    pub strict_parameters: bool,
    pub repeatable_dynamic: bool,
}
