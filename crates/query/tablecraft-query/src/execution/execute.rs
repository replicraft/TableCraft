use super::{ExecutionOptions, QueryLimits, QueryResult, Runtime};
use crate::profile::MAX_TEXT_BYTES;
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
        let _span = tracing::debug_span!("query.execute", parameters = parameters.len()).entered();
        let ceiling = QueryLimits::default();
        let l = options.limits;
        let mut budget = ResourceBudget::new(
            l.bytes.min(ceiling.bytes),
            l.rows.min(ceiling.rows),
            l.steps.min(ceiling.steps),
        );
        let mut runtime = Runtime {
            source,
            parameters: Vec::new(),
            options,
            budget: &mut budget,
        };
        runtime.tick()?;
        if parameters.len() != self.parameters.len() {
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
                || matches!(value,Value::Text(t) if t.len()>MAX_TEXT_BYTES)
                || matches!(value,Value::Single(v) if !v.to_float().is_finite())
                || matches!(value,Value::Double(v) if !v.to_float().is_finite())
            {
                return Err(QueryError::Parameter);
            }
            runtime.parameters.push(value);
        }
        let (rows, names) = match &self.body {
            PreparedBody::Read(read) => (runtime.read(read, &[])?, read.names.as_slice()),
            PreparedBody::Crosstab(cross) => {
                let result = runtime.crosstab(cross)?;
                runtime.tick()?;
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
        tracing::debug!(
            rows = rows.len(),
            columns = columns.len(),
            "query completed"
        );
        Ok(QueryResult { columns, rows })
    }
}
