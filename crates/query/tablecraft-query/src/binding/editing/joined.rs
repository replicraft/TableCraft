//! One explicitly selected local target occurrence in a scalar joined UPDATE.
use super::BoundAssignment;
use crate::binding::mutation_relation::{self, JoinedTargetSource, Target};
use crate::{
    QueryError, Source,
    binding::{Binder, Parameter, resolve},
    plan::{Expr, Relation},
};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_sql::query::{Query, Statement};

pub(crate) struct BoundJoinedUpdate {
    pub parameters: Vec<Parameter>,
    pub relation: Relation,
    pub target: Target,
    pub assignments: Vec<BoundAssignment>,
    pub filter: Option<Expr>,
}

pub(crate) fn bind_joined_update(
    query: &Query,
    source: &dyn Source,
    selected: JoinedTargetSource<'_>,
    budget: &mut ResourceBudget,
    allow_parameters: bool,
    cancelled: &dyn Fn() -> bool,
) -> Result<BoundJoinedUpdate, QueryError> {
    let Statement::Update(update) = &query.statement else {
        return Err(QueryError::Unsupported);
    };
    if update.sources.is_empty()
        || update.assignments.is_empty()
        || !allow_parameters && !query.parameters.is_empty()
    {
        return Err(QueryError::Unsupported);
    }
    if allow_parameters {
        crate::binding::mutation_parameters::admit(&query.parameters, Some(budget), cancelled)?;
    }
    let mut binder = Binder::mutation(source, budget, cancelled);
    binder.parameters = binder.bind_parameters(&query.parameters)?;
    binder.parameter_scope = binder.scope_indices(binder.parameters.len())?;
    let (relation, target) = mutation_relation::bind(&update.sources, &selected, &mut binder)?;
    binder.charge(
        update
            .assignments
            .len()
            .checked_mul(size_of::<BoundAssignment>())
            .ok_or(QueryError::StructureLimit)?,
    )?;
    let mut assignments: Vec<BoundAssignment> = Vec::new();
    assignments
        .try_reserve_exact(update.assignments.len())
        .map_err(|_| QueryError::Allocation)?;
    let scopes = [relation.slots.as_slice()];
    for assignment in &update.assignments {
        binder.work()?;
        let index = resolve(&assignment.target, &relation.slots)?.ok_or(QueryError::UnknownName)?;
        if !target.columns.contains(&index) {
            return Err(QueryError::Unsupported);
        }
        let column = index - target.columns.start;
        if let Some(work) = &mut binder.budget {
            work.charge(Resource::Steps, assignments.len() as u64)?;
        }
        if assignments.iter().any(|prior| prior.column == column) {
            return Err(QueryError::DuplicateAssignment);
        }
        let value = binder.expression(&assignment.value, &scopes, false, 1)?;
        if !crate::binding::mutation_scalar::admitted(&value) {
            return Err(QueryError::Unsupported);
        }
        assignments.push(BoundAssignment { column, value });
    }
    let filter = update
        .selection
        .as_ref()
        .map(|value| binder.expression(value, &scopes, false, 1))
        .transpose()?;
    if filter
        .as_ref()
        .is_some_and(|value| !crate::binding::mutation_scalar::predicate_admitted(value))
    {
        return Err(QueryError::Unsupported);
    }
    Ok(BoundJoinedUpdate {
        parameters: binder.parameters,
        relation,
        target,
        assignments,
        filter,
    })
}
