use super::{ExecutionOptions, QueryLimits, QueryResult, Runtime};
use crate::{PreparedQuery, QueryError, Source};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::ScalarType;

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
        let ceiling = QueryLimits::default();
        let l = options.limits;
        let mut runtime = Runtime {
            source,
            parameters: Vec::new(),
            options,
            budget: ResourceBudget::new(
                l.bytes.min(ceiling.bytes),
                l.rows.min(ceiling.rows),
                l.steps.min(ceiling.steps),
            ),
        };
        runtime.tick()?;
        if parameters.len() != self.parameters.len() {
            return Err(QueryError::Parameter);
        }
        for declared in &self.parameters {
            let mut matches = parameters
                .iter()
                .filter(|(name, _)| name.eq_ignore_ascii_case(&declared.name));
            let (_, value) = matches.next().ok_or(QueryError::Parameter)?;
            if matches.next().is_some()
                || ScalarType::of(value).is_some_and(|kind| kind != declared.kind)
                || matches!(value,Value::Text(t) if declared.max_length.is_some_and(|n|t.chars().count()>n))
                || matches!(value,Value::Text(t) if t.len()>65_536)
                || matches!(value,Value::Single(v) if !v.to_float().is_finite())
                || matches!(value,Value::Double(v) if !v.to_float().is_finite())
            {
                return Err(QueryError::Parameter);
            }
            runtime.parameters.push(value);
        }
        let rows = runtime.read(&self.read, &[])?;
        let mut columns = Vec::new();
        for name in &self.read.names {
            runtime.budget.charge(Resource::Bytes, name.len() as u64)?;
            let mut owned = String::new();
            owned
                .try_reserve_exact(name.len())
                .map_err(|_| QueryError::Allocation)?;
            owned.push_str(name);
            columns.push(owned);
        }
        Ok(QueryResult { columns, rows })
    }
}
