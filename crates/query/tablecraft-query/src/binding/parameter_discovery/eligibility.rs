//! Choose the established strict path before invoking opt-in name discovery.
use crate::{BindingDepth, QueryError, Source, plan::DomainFunction};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::{
    query::{Query, ReadBody, ReadQuery, Statement},
    select::{Expression, FunctionArgument, TableSource},
};

/// Metadata-only routing, never a permission to scan or execute a definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParameterDiscoveryMode {
    /// Preserve existing strict preparation/execution and declared prompts.
    Strict,
    /// Eligible for explicit discovery; actual binding/source errors still fail.
    Implicit,
}

/// Positively select the local metadata-only discovery profile before binding.
/// Existing named definitions, external sources, domains, crosstabs and actions
/// select Strict without trying discovery and retrying a failed preparation.
/// Source metadata errors propagate; unknown tables are checked by actual binding.
/// Source definitions must remain stable through the subsequent preparation.
/// Expression and join wrappers use a paid iterative worklist, not another depth
/// ceiling. The supplied depth applies only to nested read scopes; actual binding
/// retains its complete structural-depth and node checks.
///
/// # Errors
/// Retains source metadata failures, cancellation and the caller's work/depth bounds.
pub fn parameter_discovery_mode(
    query: &Query,
    source: &dyn Source,
    depth: BindingDepth,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<ParameterDiscoveryMode, QueryError> {
    let mut check = Check {
        source,
        budget,
        cancelled,
        depth,
    };
    check.tick()?;
    let Statement::Read(read) = &query.statement else {
        return Ok(ParameterDiscoveryMode::Strict);
    };
    let mut pending = Vec::new();
    check.push(&mut pending, Visit::Read(read, 1))?;
    while let Some(visit) = pending.pop() {
        check.tick()?;
        match visit {
            Visit::Read(read, scope) => {
                if scope > check.depth.get() {
                    return Err(QueryError::StructureLimit);
                }
                let ReadBody::Select(select) = &read.body else {
                    return Ok(ParameterDiscoveryMode::Strict);
                };
                if read.owner_access
                    || select.owner_access
                    || select.external.is_some()
                    || select.into.is_some()
                    || select.into_external.is_some()
                {
                    return Ok(ParameterDiscoveryMode::Strict);
                }
                for key in read.order_by.iter().chain(select.order_by.iter()).rev() {
                    check.push(&mut pending, Visit::Expression(&key.expression, scope))?;
                }
                for value in select
                    .selection
                    .iter()
                    .chain(select.having.iter())
                    .chain(select.group_by.iter())
                    .rev()
                {
                    check.push(&mut pending, Visit::Expression(value, scope))?;
                }
                for projection in select.projection.iter().rev() {
                    check.tick()?;
                    if let tablecraft_sql::select::Projection::Expression { expression, .. } =
                        projection
                    {
                        check.push(&mut pending, Visit::Expression(expression, scope))?;
                    }
                }
                for source in select.from.iter().rev() {
                    check.push(&mut pending, Visit::Table(source, scope))?;
                }
            }
            Visit::Table(table, scope) => match table {
                TableSource::Table(table) => {
                    if table.database.is_some()
                        || check.source.query(table.name.as_str())?.is_some()
                    {
                        return Ok(ParameterDiscoveryMode::Strict);
                    }
                    check.tick()?;
                }
                TableSource::Join {
                    left,
                    right,
                    condition,
                    ..
                } => {
                    check.push(&mut pending, Visit::Expression(condition, scope))?;
                    check.push(&mut pending, Visit::Table(right, scope))?;
                    check.push(&mut pending, Visit::Table(left, scope))?;
                }
                TableSource::Group { source, .. } => {
                    check.push(&mut pending, Visit::Table(source, scope))?
                }
                TableSource::Derived { query, .. } => {
                    check.push(&mut pending, Visit::Read(query, nested(scope)?))?
                }
            },
            Visit::Expression(value, scope) => match value {
                Expression::Call {
                    qualifier,
                    name,
                    arguments,
                } => {
                    if !qualifier.is_empty() || DomainFunction::resolve(name.as_str()).is_some() {
                        return Ok(ParameterDiscoveryMode::Strict);
                    }
                    for argument in arguments.iter().rev() {
                        check.tick()?;
                        if let FunctionArgument::Expression(value) = argument {
                            check.push(&mut pending, Visit::Expression(value, scope))?;
                        }
                    }
                }
                Expression::Unary { expression, .. }
                | Expression::IsNull { expression, .. }
                | Expression::Member { expression, .. } => {
                    check.push(&mut pending, Visit::Expression(expression, scope))?
                }
                Expression::Binary { left, right, .. } => {
                    check.push(&mut pending, Visit::Expression(right, scope))?;
                    check.push(&mut pending, Visit::Expression(left, scope))?;
                }
                Expression::Like {
                    expression,
                    pattern,
                    ..
                } => {
                    check.push(&mut pending, Visit::Expression(pattern, scope))?;
                    check.push(&mut pending, Visit::Expression(expression, scope))?;
                }
                Expression::Between {
                    expression,
                    first_bound,
                    second_bound,
                    ..
                } => {
                    check.push(&mut pending, Visit::Expression(second_bound, scope))?;
                    check.push(&mut pending, Visit::Expression(first_bound, scope))?;
                    check.push(&mut pending, Visit::Expression(expression, scope))?;
                }
                Expression::InList {
                    expression, values, ..
                } => {
                    for value in values.iter().rev() {
                        check.push(&mut pending, Visit::Expression(value, scope))?;
                    }
                    check.push(&mut pending, Visit::Expression(expression, scope))?;
                }
                Expression::Exists(query) | Expression::Subquery(query) => {
                    check.push(&mut pending, Visit::Read(query, nested(scope)?))?
                }
                Expression::InQuery {
                    expression, query, ..
                }
                | Expression::Quantified {
                    expression, query, ..
                } => {
                    check.push(&mut pending, Visit::Read(query, nested(scope)?))?;
                    check.push(&mut pending, Visit::Expression(expression, scope))?;
                }
                _ => {}
            },
        }
    }
    Ok(ParameterDiscoveryMode::Implicit)
}

enum Visit<'a> {
    Read(&'a ReadQuery, usize),
    Table(&'a TableSource, usize),
    Expression(&'a Expression, usize),
}

fn nested(scope: usize) -> Result<usize, QueryError> {
    scope.checked_add(1).ok_or(QueryError::StructureLimit)
}

struct Check<'a> {
    source: &'a dyn Source,
    budget: &'a mut ResourceBudget,
    cancelled: &'a dyn Fn() -> bool,
    depth: BindingDepth,
}

impl Check<'_> {
    fn tick(&mut self) -> Result<(), QueryError> {
        if (self.cancelled)() {
            return Err(QueryError::Cancelled);
        }
        self.budget.charge(Resource::Steps, 1)?;
        Ok(())
    }

    fn push<'a>(
        &mut self,
        pending: &mut Vec<Visit<'a>>,
        value: Visit<'a>,
    ) -> Result<(), QueryError> {
        self.tick()?;
        if pending.len() == pending.capacity() {
            let added = pending.capacity().max(1);
            self.budget.charge(Resource::Steps, pending.len() as u64)?;
            self.budget
                .charge_allocation(added as u64, size_of::<Visit<'a>>() as u64)?;
            pending
                .try_reserve_exact(added)
                .map_err(|_| QueryError::Allocation)?;
        }
        pending.push(value);
        Ok(())
    }
}
