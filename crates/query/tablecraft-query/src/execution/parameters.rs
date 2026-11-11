//! Shared strict typed parameter binding for retrieval and frozen mutations.
use super::{ExecutionOptions, Runtime, context::ParameterValue};
use crate::{
    QueryError,
    binding::{Parameter, ValuesSource, bind_declared_parameters},
    discovery::DiscoveryState,
};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_sql::query::Query;

impl<'a> Runtime<'a, '_> {
    pub(super) fn bind_parameters(
        &mut self,
        declarations: &[Parameter],
        supplied: &'a [(&str, Value)],
        strict: bool,
    ) -> Result<(), QueryError> {
        if strict && supplied.len() != declarations.len() {
            return Err(QueryError::Parameter);
        }
        self.budget.charge_allocation(
            declarations.len() as u64,
            size_of::<ParameterValue<'_>>() as u64,
        )?;
        self.parameters
            .try_reserve_exact(declarations.len())
            .map_err(|_| QueryError::Allocation)?;

        for declared in declarations {
            self.tick()?;
            let mut selected = None;
            for (name, value) in supplied {
                self.tick()?;
                let work = name
                    .len()
                    .checked_add(declared.name.as_str().len())
                    .ok_or(QueryError::StructureLimit)?;
                self.budget.charge(Resource::Steps, work as u64)?;
                if crate::profile::SOURCE_SCHEMA_POLICY
                    .names
                    .equals(name, declared.name.as_str())
                {
                    if selected.is_some() {
                        return Err(QueryError::Parameter);
                    }
                    selected = Some(value);
                }
            }
            let value = selected.ok_or(QueryError::Parameter)?;
            if let Value::Text(text) = value {
                if text.len() > self.options.text_limit() {
                    return Err(QueryError::Parameter);
                }
                // Declared text constraints can inspect the complete UTF-8 value.
                self.budget.charge(Resource::Steps, text.len() as u64)?;
            }
            if !declared.data_type.accepts(value)
                || matches!(value, Value::Single(value) if !value.to_float().is_finite())
                || matches!(value, Value::Double(value) if !value.to_float().is_finite())
            {
                return Err(QueryError::Parameter);
            }

            self.parameters.push(ParameterValue {
                value,
                declaration: declared.data_type.declaration(),
            });
        }
        Ok(())
    }
}

/// Root input errors precede staged schema-discovery row reads. The source
/// here cannot service relation metadata or scans; only declarations are bound.
pub(crate) fn validate_staged_parameters<'a>(
    query: &Query,
    supplied: &'a [(&str, Value)],
    options: ExecutionOptions<'a>,
    budget: &mut ResourceBudget,
) -> Result<(), QueryError> {
    let never_cancelled = || false;
    let cancelled = options.cancelled.unwrap_or(&never_cancelled);
    let source = ValuesSource;
    let declarations = bind_declared_parameters(&query.parameters, &source, budget, cancelled)?;
    let discovery = DiscoveryState::new(false);
    let mut runtime = Runtime {
        owner: crate::external_sources::SourceNamespace::Root,
        numeric_scratch: Vec::new(),
        repeatable_dynamic: false,
        source: &source,
        external_sources: Vec::new(),
        discovery: &discovery,
        parameters: Vec::new(),
        options,
        budget,
    };
    runtime.bind_parameters(&declarations, supplied, true)
}
