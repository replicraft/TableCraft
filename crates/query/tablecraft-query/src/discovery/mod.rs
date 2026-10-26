//! Explicit execution staging for named crosstab schemas; binding never scans.
mod bindings;
use crate::{
    ExecutionOptions, ExpressionQueryResult, PreparedQuery, QueryError, QueryLimits, Source,
    binding::{PreparationOptions, StageOptions, prepare_stage},
    execution::{CellQueryResult, ExecutionContext, publish_cells},
    plan::PreparedBody,
    profile::MAX_NAMED_DEPTH,
    source::TextPolicy,
};
use crate::{external_sources::SourceNamespace, named_identity::NamedIdentity};
use std::cell::{Cell, RefCell};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_sql::query::{Query, Statement};

// Independent dependency-count ceiling, in addition to nesting and shared quotas.
const MAX_DISCOVERED_QUERIES: usize = 256;

pub(crate) struct Materialized<'a> {
    pub namespace: SourceNamespace,
    pub name: String,
    pub definition: &'a Query,
    pub result: CellQueryResult,
    pub policies: Vec<TextPolicy>,
}

/// Run a read query with explicit, bounded named-crosstab schema discovery.
/// Discovery completes each required crosstab once and reuses its buffered data
/// within this call. Source schemas/definitions/data must remain stable throughout.
/// All stages share limits and execution providers; no state survives this call.
/// `prepare` remains metadata-only and does not perform this staging implicitly.
/// Discovery handles relational dependencies and parameterless runtime domain
/// targets. Later binding errors may follow discovery reads, but no partial result
/// is published.
pub fn execute_with_schema_discovery<'a>(
    query: &Query,
    source: &'a dyn Source,
    parameters: &'a [(&str, Value)],
    options: ExecutionOptions<'a>,
) -> Result<ExpressionQueryResult, QueryError> {
    let _span = tracing::debug_span!("query.discover.execute").entered();
    let mut budget = budget(options.limits);
    let discovery = DiscoveryState::new(true);
    let mut operation = Operation {
        source,
        parameters,
        options,
        budget: &mut budget,
        discovery: &discovery,
        parameterless: false,
        repeatable_dynamic: false,
    };
    let plan = operation.prepare(query, SourceNamespace::Root)?;
    let result = plan.execute_discovered(
        source,
        parameters,
        options,
        operation.budget,
        &discovery,
        ExecutionContext {
            strict_parameters: true,
            repeatable_dynamic: false,
        },
    )?;
    publish_cells(result, operation.budget, options)
}

/// Execute with explicit structured multivalue results and the same bounded
/// execution-phase schema discovery used by scalar retrieval. Static named and
/// derived list projections retain complete lists; unsupported typed shapes
/// never substitute a scalar or cache a partial relation.
pub fn execute_collections_with_schema_discovery<'a>(
    query: &Query,
    source: &'a dyn Source,
    parameters: &'a [(&str, Value)],
    options: ExecutionOptions<'a>,
) -> Result<crate::CollectionQueryResult, QueryError> {
    let _span = tracing::debug_span!("query.discover.execute").entered();
    let mut budget = budget(options.limits);
    let discovery = DiscoveryState::new(true);
    let mut operation = Operation {
        source,
        parameters,
        options,
        budget: &mut budget,
        discovery: &discovery,
        parameterless: false,
        repeatable_dynamic: false,
    };
    let plan = operation.prepare(query, SourceNamespace::Root)?;
    plan.execute_collections_discovered(
        source,
        parameters,
        options,
        operation.budget,
        &discovery,
        ExecutionContext {
            strict_parameters: true,
            repeatable_dynamic: false,
        },
    )
}

pub(crate) fn budget(limits: QueryLimits) -> ResourceBudget {
    ResourceBudget::new(
        limits.bytes.min(QueryLimits::MAX_BYTES),
        limits.rows.min(QueryLimits::MAX_ROWS),
        limits.steps.min(QueryLimits::MAX_STEPS),
    )
}

/// Shared only within one synchronous execution. Borrows are confined to metadata
/// binding or copying completed cells, never held across expression evaluation.
pub(crate) struct DiscoveryState<'source> {
    pub(crate) active_read_depth: Cell<usize>,
    pub(crate) active_domain_depth: Cell<usize>,
    pub materialized: RefCell<Vec<Materialized<'source>>>,
    pub(crate) external_sources: RefCell<Vec<crate::external_sources::ExternalBinding>>,
    active: RefCell<Vec<NamedIdentity>>,
    pub enabled: bool,
}

impl<'source> DiscoveryState<'source> {
    pub(crate) fn new(enabled: bool) -> Self {
        Self {
            active_read_depth: Cell::new(0),
            active_domain_depth: Cell::new(0),
            materialized: RefCell::new(Vec::new()),
            external_sources: RefCell::new(Vec::new()),
            active: RefCell::new(Vec::new()),
            enabled,
        }
    }

    pub(crate) fn discover_domain(
        &self,
        name: NamedIdentity,
        source: &'source dyn Source,
        options: ExecutionOptions<'_>,
        budget: &mut ResourceBudget,
        repeatable_dynamic: bool,
    ) -> Result<(), QueryError> {
        if !self.enabled {
            return Err(QueryError::Unsupported);
        }
        Operation {
            source,
            parameters: &[],
            options,
            budget,
            discovery: self,
            parameterless: true,
            repeatable_dynamic,
        }
        .discover(name)
    }
}

struct Operation<'a, 'source> {
    source: &'source dyn Source,
    parameters: &'a [(&'a str, Value)],
    options: ExecutionOptions<'a>,
    budget: &'a mut ResourceBudget,
    discovery: &'a DiscoveryState<'source>,
    parameterless: bool,
    repeatable_dynamic: bool,
}

impl<'source> Operation<'_, 'source> {
    fn tick(&mut self) -> Result<(), QueryError> {
        if self.options.cancelled.is_some_and(|cancel| cancel()) {
            return Err(QueryError::Cancelled);
        }
        self.budget.charge(Resource::Steps, 1)?;
        Ok(())
    }

    fn prepare(
        &mut self,
        query: &Query,
        owner: SourceNamespace,
    ) -> Result<PreparedQuery, QueryError> {
        loop {
            self.tick()?;
            let stage = prepare_stage(
                query,
                self.source,
                &self.discovery.materialized.borrow(),
                Some(self.budget),
                self.options.cancelled,
                StageOptions {
                    inherited_read_depth: self.discovery.active_read_depth.get(),
                    owner,
                    external_sources: &self.discovery.external_sources.borrow(),
                    preparation: PreparationOptions {
                        binding_depth: self.options.binding_depth,
                        reference_year: self.options.reference_year,
                        declared_arithmetic: self.options.declared_integral,
                    },
                    discover: true,
                    parameterless: self.parameterless,
                },
            )?;
            match stage {
                Ok(plan) => return Ok(plan),
                Err(request) => {
                    self.discovery.remember_bindings(
                        request.external_sources,
                        self.budget,
                        self.options.cancelled,
                    )?;
                    self.discover(request.identity)?;
                }
            }
        }
    }

    fn discover(&mut self, name: NamedIdentity) -> Result<(), QueryError> {
        self.tick()?;
        for active in self.discovery.active.borrow().iter() {
            self.budget.charge(
                Resource::Steps,
                (active.name.len() + name.name.len()) as u64,
            )?;
            if active.matches(name.namespace, &name.name) {
                return Err(QueryError::DependencyCycle);
            }
        }
        if self.discovery.active.borrow().len() >= MAX_NAMED_DEPTH
            || self.discovery.materialized.borrow().len() >= MAX_DISCOVERED_QUERIES
        {
            return Err(QueryError::StructureLimit);
        }
        self.budget
            .charge(Resource::Bytes, size_of::<NamedIdentity>() as u64)?;
        {
            let mut active = self.discovery.active.borrow_mut();
            active.try_reserve(1).map_err(|_| QueryError::Allocation)?;
            active.push(name);
        }
        let result = self.materialize_active();
        let name = self
            .discovery
            .active
            .borrow_mut()
            .pop()
            .ok_or(QueryError::Source)?;
        let (definition, result, policies) = result?;
        self.tick()?;
        if self.discovery.materialized.borrow().len() >= MAX_DISCOVERED_QUERIES {
            return Err(QueryError::StructureLimit);
        }
        self.budget
            .charge(Resource::Bytes, size_of::<Materialized<'_>>() as u64)?;
        let mut materialized = self.discovery.materialized.borrow_mut();
        materialized
            .try_reserve(1)
            .map_err(|_| QueryError::Allocation)?;
        materialized.push(Materialized {
            namespace: name.namespace,
            name: name.name,
            definition,
            result,
            policies,
        });
        Ok(())
    }

    fn source_for(
        &mut self,
        namespace: SourceNamespace,
    ) -> Result<&'source dyn Source, QueryError> {
        self.tick()?;
        if namespace == SourceNamespace::Root {
            return Ok(self.source);
        }
        let discovery = self.discovery;
        let bindings = discovery.external_sources.borrow();
        for binding in bindings.iter() {
            self.tick()?;
            if namespace != SourceNamespace::External(binding.key) {
                continue;
            }
            self.budget
                .charge(Resource::Steps, binding.comparison_work(binding.request())?)?;
            let root = self.source;
            let resolved = root
                .external_source(binding.request())?
                .ok_or(QueryError::SchemaChanged)?;
            self.tick()?;
            if resolved.key() != binding.key {
                return Err(QueryError::SchemaChanged);
            }
            return Ok(resolved.source());
        }
        Err(QueryError::SchemaChanged)
    }

    fn materialize_active(
        &mut self,
    ) -> Result<(&'source Query, CellQueryResult, Vec<TextPolicy>), QueryError> {
        let discovery = self.discovery;
        let (owner, definition) = {
            let active = discovery.active.borrow();
            let identity = active.last().ok_or(QueryError::Source)?;
            let provider = self.source_for(identity.namespace)?;
            let definition = provider
                .query(&identity.name)?
                .ok_or(QueryError::UnknownName)?;
            (identity.namespace, definition)
        };
        if self.parameterless && !definition.parameters.is_empty() {
            return Err(QueryError::Parameter);
        }
        if !matches!(definition.statement, Statement::Crosstab(_)) {
            return Err(QueryError::SchemaChanged);
        }
        let plan = self.prepare(definition, owner)?;
        let result = plan.execute_discovered(
            self.source,
            self.parameters,
            self.options,
            self.budget,
            self.discovery,
            ExecutionContext {
                strict_parameters: false,
                repeatable_dynamic: self.repeatable_dynamic,
            },
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
