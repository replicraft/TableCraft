//! Explicit metadata-only unresolved-name discovery; strict preparation is unchanged.
mod eligibility;
use super::{
    Binder, Parameter, ParameterType, PreparationOptions, PreparedQuery, StageOptions,
    prepare_stage,
};
use crate::{
    CollectionQueryResult, ExecutionOptions, QueryError, Source,
    profile::{MAX_PARAMETERS, SOURCE_SCHEMA_POLICY},
};
pub use eligibility::{ParameterDiscoveryMode, parameter_discovery_mode};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};
use tablecraft_schema::SchemaName;
use tablecraft_sql::query::{Query, Statement};

/// Why a prepared input exists; this never claims inferred native typing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterOrigin {
    /// An explicit PARAMETERS declaration retains its exact type constraints.
    Declared,
    /// An unresolved unqualified scalar name, retaining the selected VALUE carrier.
    Implicit,
}

/// Borrowed metadata from the actual bound parameter owner, without copied payloads.
#[derive(Clone, Copy)]
pub struct DiscoveredParameter<'a> {
    /// Original first spelling; lookup uses the query's existing name policy.
    pub name: &'a str,
    /// Exact declared type or explicit carrier-preserving VALUE extension.
    pub data_type: ParameterType,
    /// Distinguishes declarations from unresolved scalar identifiers.
    pub origin: ParameterOrigin,
}

/// Captured parameter nodes and source schemas, retaining no native source borrow.
/// Discovery does not grant source opens, row reads or mutation authority.
#[must_use = "execute explicitly or discard the discovered query"]
pub struct PreparedParameterQuery {
    prepared: PreparedQuery,
    declared: usize,
}

impl PreparedParameterQuery {
    /// Declared inputs first, then implicit inputs in binding encounter order.
    pub fn parameters(&self) -> impl ExactSizeIterator<Item = DiscoveredParameter<'_>> {
        self.prepared
            .parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| DiscoveredParameter {
                name: parameter.name.as_str(),
                data_type: parameter.data_type,
                origin: if index < self.declared {
                    ParameterOrigin::Declared
                } else {
                    ParameterOrigin::Implicit
                },
            })
    }

    /// Execute captured parameter nodes with the existing complete result contract.
    /// All source schemas are revalidated; no SQL text or AST is reconstructed.
    ///
    /// # Errors
    /// Retains existing input/schema/source/evaluation, budget and cancellation errors.
    pub fn execute_collections<'a>(
        &self,
        source: &'a dyn Source,
        parameters: &'a [(&str, Value)],
        options: ExecutionOptions<'a>,
    ) -> Result<CollectionQueryResult, QueryError> {
        self.prepared
            .execute_collections(source, parameters, options)
    }
}

/// Bind local SELECT queries while explicitly discovering unresolved unqualified
/// scalar names. Ordinary fields, unavailable/ambiguous fields, outer scopes,
/// declarations and eligible projection aliases retain existing resolution order.
/// Implicit VALUE inputs preserve a caller-selected carrier; no native contextual
/// type inference or expression coercion is claimed. No row or function is sampled.
/// Named definitions, external sources, domains, crosstabs and actions stay closed.
///
/// # Errors
/// Retains typed metadata/policy/source errors, invalid names/declarations,
/// unsupported profiles, allocation/work exhaustion and cancellation.
pub fn prepare_with_parameter_discovery(
    query: &Query,
    source: &dyn Source,
    options: PreparationOptions,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<PreparedParameterQuery, QueryError> {
    if cancelled() {
        return Err(QueryError::Cancelled);
    }
    let Statement::Read(_) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    if parameter_discovery_mode(query, source, options.binding_depth, budget, cancelled)?
        != ParameterDiscoveryMode::Implicit
    {
        return Err(QueryError::Unsupported);
    }
    let prepared = prepare_stage(
        query,
        source,
        &[],
        Some(budget),
        Some(cancelled),
        StageOptions {
            preparation: options,
            implicit_parameters: true,
            ..Default::default()
        },
    )?
    .map_err(|_| QueryError::Unsupported)?;
    Ok(PreparedParameterQuery {
        prepared,
        declared: query.parameters.len(),
    })
}

impl Binder<'_> {
    pub(super) fn implicit_parameter(
        &mut self,
        name: &str,
    ) -> Result<Option<crate::plan::Expr>, QueryError> {
        let Some(start) = self.implicit_parameter_start else {
            return Ok(None);
        };
        for index in start..self.parameters.len() {
            self.work()?;
            if SOURCE_SCHEMA_POLICY
                .names
                .equals(self.parameters[index].name.as_str(), name)
            {
                return Ok(Some(crate::plan::Expr::Parameter(index)));
            }
        }
        self.work()?;
        if self.parameters.len() == MAX_PARAMETERS
            || name.len() > SOURCE_SCHEMA_POLICY.max_name_bytes
        {
            return Err(QueryError::StructureLimit);
        }
        self.charge(name.len())?;
        let mut owned = String::new();
        owned
            .try_reserve_exact(name.len())
            .map_err(|_| QueryError::Allocation)?;
        owned.push_str(name);
        if self.parameters.len() == self.parameters.capacity() {
            let added = self.parameters.capacity().max(1);
            if let Some(budget) = &mut self.budget {
                budget.charge(Resource::Steps, self.parameters.len() as u64)?;
            }
            self.charge(
                added
                    .checked_mul(size_of::<Parameter>())
                    .ok_or(QueryError::StructureLimit)?,
            )?;
            self.parameters
                .try_reserve_exact(added)
                .map_err(|_| QueryError::Allocation)?;
        }
        let index = self.parameters.len();
        self.parameters.push(Parameter {
            name: SchemaName::new(owned)?,
            data_type: ParameterType::Value,
        });
        // Implicit indices deliberately stay out of declared parameter_scope:
        // eligible projection aliases must resolve before implicit identities.
        Ok(Some(crate::plan::Expr::Parameter(index)))
    }
}

#[cfg(test)]
mod tests;
