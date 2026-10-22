//! Explicit execution staging for named crosstab schemas; binding never scans.
use crate::{
    ExecutionOptions, ExpressionQueryResult, PreparedQuery, QueryError, QueryLimits, Source,
    binding::prepare_stage,
    plan::PreparedBody,
    profile::{MAX_NAMED_DEPTH, SOURCE_SCHEMA_POLICY},
    source::TextPolicy,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_sql::query::{Query, Statement};

// Independent dependency-count ceiling, in addition to nesting and shared quotas.
const MAX_DISCOVERED_QUERIES: usize = 256;

pub(crate) struct Materialized<'a> {
    pub name: String,
    pub definition: &'a Query,
    pub result: ExpressionQueryResult,
    pub policies: Vec<TextPolicy>,
}

/// Run a read query with explicit, bounded named-crosstab schema discovery.
/// Discovery completes each required crosstab once and reuses its buffered data
/// within this call. Source schemas/definitions/data must remain stable throughout.
/// All stages share limits and execution providers; no state survives this call.
/// `prepare` remains metadata-only and does not perform this staging implicitly.
/// Discovery handles relational dependencies; runtime domain-function targets
/// retain their previous profile. Later outer binding errors may follow discovery
/// reads, but no partial result is published.
pub fn execute_with_schema_discovery<'a>(
    query: &Query,
    source: &'a dyn Source,
    parameters: &'a [(&str, Value)],
    options: ExecutionOptions<'a>,
) -> Result<ExpressionQueryResult, QueryError> {
    let _span = tracing::debug_span!("query.discover.execute").entered();
    let mut operation = Operation {
        source,
        parameters,
        options,
        budget: budget(options.limits),
        materialized: Vec::new(),
        active: Vec::new(),
    };
    let plan = operation.prepare(query)?;
    plan.execute_discovered(
        source,
        parameters,
        options,
        &mut operation.budget,
        &operation.materialized,
        true,
    )
}

pub(crate) fn budget(limits: QueryLimits) -> ResourceBudget {
    ResourceBudget::new(
        limits.bytes.min(QueryLimits::MAX_BYTES),
        limits.rows.min(QueryLimits::MAX_ROWS),
        limits.steps.min(QueryLimits::MAX_STEPS),
    )
}

struct Operation<'a> {
    source: &'a dyn Source,
    parameters: &'a [(&'a str, Value)],
    options: ExecutionOptions<'a>,
    budget: ResourceBudget,
    materialized: Vec<Materialized<'a>>,
    active: Vec<String>,
}

impl<'a> Operation<'a> {
    fn tick(&mut self) -> Result<(), QueryError> {
        if self.options.cancelled.is_some_and(|cancel| cancel()) {
            return Err(QueryError::Cancelled);
        }
        self.budget.charge(Resource::Steps, 1)?;
        Ok(())
    }

    fn prepare(&mut self, query: &Query) -> Result<PreparedQuery, QueryError> {
        loop {
            self.tick()?;
            match prepare_stage(
                query,
                self.source,
                &self.materialized,
                Some(&mut self.budget),
                self.options.cancelled,
                true,
            )? {
                Ok(plan) => return Ok(plan),
                Err(name) => self.discover(name)?,
            }
        }
    }

    fn discover(&mut self, name: String) -> Result<(), QueryError> {
        self.tick()?;
        for active in &self.active {
            self.budget
                .charge(Resource::Steps, (active.len() + name.len()) as u64)?;
            if SOURCE_SCHEMA_POLICY.names.equals(active, &name) {
                return Err(QueryError::DependencyCycle);
            }
        }
        if self.active.len() >= MAX_NAMED_DEPTH || self.materialized.len() >= MAX_DISCOVERED_QUERIES
        {
            return Err(QueryError::StructureLimit);
        }
        self.budget
            .charge(Resource::Bytes, size_of::<String>() as u64)?;
        self.active
            .try_reserve(1)
            .map_err(|_| QueryError::Allocation)?;
        self.active.push(name);
        let result = self.materialize_active();
        let name = self.active.pop().ok_or(QueryError::Source)?;
        let (definition, result, policies) = result?;
        self.tick()?;
        if self.materialized.len() >= MAX_DISCOVERED_QUERIES {
            return Err(QueryError::StructureLimit);
        }
        self.budget
            .charge(Resource::Bytes, size_of::<Materialized<'_>>() as u64)?;
        self.materialized
            .try_reserve(1)
            .map_err(|_| QueryError::Allocation)?;
        self.materialized.push(Materialized {
            name,
            definition,
            result,
            policies,
        });
        Ok(())
    }

    fn materialize_active(
        &mut self,
    ) -> Result<(&'a Query, ExpressionQueryResult, Vec<TextPolicy>), QueryError> {
        let name = self.active.last().ok_or(QueryError::Source)?;
        let definition = self.source.query(name)?.ok_or(QueryError::UnknownName)?;
        if !matches!(definition.statement, Statement::Crosstab(_)) {
            return Err(QueryError::SchemaChanged);
        }
        let plan = self.prepare(definition)?;
        let result = plan.execute_discovered(
            self.source,
            self.parameters,
            self.options,
            &mut self.budget,
            &self.materialized,
            false,
        )?;
        let PreparedBody::Crosstab(cross) = &plan.body else {
            return Err(QueryError::SchemaChanged);
        };
        let width = cross
            .read
            .names
            .len()
            .checked_sub(1)
            .ok_or(QueryError::Width)?;
        let heading_count = result
            .columns()
            .len()
            .checked_sub(width)
            .ok_or(QueryError::Width)?;
        self.budget.charge_allocation(
            result.columns().len() as u64,
            size_of::<TextPolicy>() as u64,
        )?;
        let mut policies = Vec::new();
        policies
            .try_reserve_exact(result.columns().len())
            .map_err(|_| QueryError::Allocation)?;
        policies.extend_from_slice(&cross.read.text_policies[..width]);
        let cell = *cross.read.text_policies.last().ok_or(QueryError::Width)?;
        policies.extend(std::iter::repeat_n(cell, heading_count));
        Ok((definition, result, policies))
    }
}
