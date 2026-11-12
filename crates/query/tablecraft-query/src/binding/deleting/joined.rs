//! Explicit DISTINCTROW selects one original target occurrence, never a value key.
use crate::{
    QueryError, Source,
    binding::{
        Binder, Parameter,
        mutation_relation::{self, JoinedTargetSource, Target},
    },
    plan::{Expr, Relation},
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::{
    query::{Delete, Query, Statement},
    select::TableSource,
};

pub(crate) struct BoundJoinedDelete {
    pub parameters: Vec<Parameter>,
    pub relation: Relation,
    pub target: Target,
    pub filter: Option<Expr>,
}

pub(crate) fn admit<'a>(
    query: &'a Query,
    qualifier: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<&'a Delete, QueryError> {
    tick(budget, cancelled)?;
    let Statement::Delete(delete) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    let [target] = delete.targets.as_slice() else {
        return Err(QueryError::Unsupported);
    };
    let [source] = delete.sources.as_slice() else {
        return Err(QueryError::Unsupported);
    };
    budget.charge(
        Resource::Steps,
        target.as_str().len().max(qualifier.len()) as u64,
    )?;
    if !delete.distinct_row || !target.as_str().eq_ignore_ascii_case(qualifier) {
        return Err(QueryError::Unsupported);
    }
    let mut pending = Vec::new();
    push(&mut pending, source, budget, cancelled)?;
    let mut joined = false;
    while let Some(source) = pending.pop() {
        tick(budget, cancelled)?;
        match source {
            TableSource::Table(table) if table.database.is_none() => {}
            TableSource::Join {
                left,
                right,
                condition,
                ..
            } => {
                crate::binding::mutation_reads::syntax::assignment(condition, budget, cancelled)?;
                joined = true;
                push(&mut pending, right, budget, cancelled)?;
                push(&mut pending, left, budget, cancelled)?;
            }
            TableSource::Group {
                source,
                alias: None,
            } => push(&mut pending, source, budget, cancelled)?,
            _ => return Err(QueryError::Unsupported),
        }
    }
    if !joined {
        return Err(QueryError::Unsupported);
    }
    if let Some(filter) = &delete.selection {
        crate::binding::mutation_reads::syntax::assignment(filter, budget, cancelled)?;
    }
    Ok(delete)
}

pub(crate) fn bind(
    query: &Query,
    source: &dyn Source,
    selected: JoinedTargetSource<'_>,
    budget: &mut ResourceBudget,
    allow_parameters: bool,
    cancelled: &dyn Fn() -> bool,
) -> Result<BoundJoinedDelete, QueryError> {
    let delete = admit(query, selected.qualifier, budget, cancelled)?;
    if !allow_parameters && !query.parameters.is_empty() {
        return Err(QueryError::Unsupported);
    }
    if allow_parameters {
        crate::binding::mutation_parameters::admit(&query.parameters, Some(budget), cancelled)?;
    }
    let mut binder = Binder::mutation(source, budget, cancelled);
    binder.parameters = binder.bind_parameters(&query.parameters)?;
    binder.parameter_scope = binder.scope_indices(binder.parameters.len())?;
    let (relation, target) = mutation_relation::bind(&delete.sources, &selected, &mut binder)?;
    let scopes = [relation.slots.as_slice()];
    let filter = delete
        .selection
        .as_ref()
        .map(|expression| binder.expression(expression, &scopes, false, 1))
        .transpose()?;
    if filter
        .as_ref()
        .is_some_and(|value| !crate::binding::mutation_scalar::admitted(value))
    {
        return Err(QueryError::Unsupported);
    }
    Ok(BoundJoinedDelete {
        parameters: binder.parameters,
        relation,
        target,
        filter,
    })
}

fn tick(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), QueryError> {
    if cancelled() {
        return Err(QueryError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}

fn push<'a>(
    pending: &mut Vec<&'a TableSource>,
    source: &'a TableSource,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), QueryError> {
    tick(budget, cancelled)?;
    if pending.len() == pending.capacity() {
        let additional = pending.capacity().max(1);
        budget.charge_allocation(additional as u64, size_of::<&TableSource>() as u64)?;
        budget.charge(Resource::Steps, pending.len() as u64)?;
        pending
            .try_reserve_exact(additional)
            .map_err(|_| QueryError::Allocation)?;
    }
    pending.push(source);
    Ok(())
}
