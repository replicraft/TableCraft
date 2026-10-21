//! Runtime domain preparation uses the selected Source and shared operation budget.
mod scalar;
mod text;
use super::{Binder, projection};
use crate::{
    QueryError, Source,
    plan::{DomainFunction, Expr, ReadNode, ReadPlan, SelectPlan, Slot},
};
use tablecraft_core::limits::ResourceBudget;

pub(crate) fn bind_domain<'a>(
    source: &'a dyn Source,
    kind: DomainFunction,
    expression: &str,
    domain: &str,
    criteria: Option<&str>,
    budget: &'a mut ResourceBudget,
    cancelled: Option<&'a dyn Fn() -> bool>,
) -> Result<Option<ReadPlan>, QueryError> {
    let (syntax, value, filter) = {
        let mut text = text::Text { budget, cancelled };
        let syntax = text.source(domain)?;
        let value = if expression == "*" && kind == DomainFunction::Count {
            None
        } else {
            Some(text.expression(expression)?)
        };
        let filter = criteria
            .filter(|s| !s.trim().is_empty())
            .map(|s| text.expression(s))
            .transpose()?;
        (syntax, value, filter)
    };
    let mut binder = Binder {
        source,
        budget: Some(budget),
        cancelled,
        parameters: Vec::new(),
        parameter_scope: Vec::new(),
        retrieval: true,
        active_queries: Vec::new(),
        nodes: 0,
        policy_visits: 0,
        bytes: 0,
    };
    let relation = binder.relation(syntax.from.first().ok_or(QueryError::Unsupported)?, &[], 1)?;
    if !binder.parameters.is_empty() {
        return Err(QueryError::Parameter);
    }
    // The initial domain profile uses scalar declarations. Ordinary scalar
    // subsets of mixed native tables remain valid without expanding children.
    let scopes: [&[Slot]; 1] = [&relation.slots];
    let value = value
        .as_ref()
        .map(|e| binder.expression(e, &scopes, false, 1))
        .transpose()?;
    if let Some(value) = &value {
        binder.domain_scalar(value, &relation.slots, false, 1)?;
    }
    let filter = match filter
        .as_ref()
        .map(|e| binder.expression(e, &scopes, false, 1))
        .transpose()
    {
        Ok(value) => value,
        Err(QueryError::UnknownName) => return Ok(None),
        Err(error) => return Err(error),
    };
    if let Some(filter) = &filter {
        binder.domain_scalar(filter, &relation.slots, true, 1)?;
    }
    if !binder.parameters.is_empty() {
        return Err(QueryError::Parameter);
    }
    let argument = value.map(Box::new);
    let expr = Expr::Aggregate(kind.aggregate(), argument);
    let policy = binder.text_policy(&expr)?;
    binder.charge(size_of::<String>())?;
    let mut names = Vec::new();
    names
        .try_reserve_exact(1)
        .map_err(|_| QueryError::Allocation)?;
    names.push(binder.copy_name("value")?);
    binder.charge(
        size_of::<SelectPlan>() + size_of::<ReadPlan>() + size_of::<Expr>() + size_of_val(&policy),
    )?;
    let mut policies = binder.policy_vector(1)?;
    policies.push(policy);
    let mut projection = Vec::new();
    projection
        .try_reserve_exact(1)
        .map_err(|_| QueryError::Allocation)?;
    projection.push(expr);
    let mut plan = ReadPlan {
        names,
        text_policies: policies,
        node: ReadNode::Select(Box::new(SelectPlan {
            source: relation,
            projection,
            filter,
            group: Vec::new(),
            having: None,
            aggregate: true,
            record_keys: Vec::new(),
            parent: None,
        })),
        sort: Vec::new(),
        distinct: false,
        top: None,
    };
    projection::prune(&mut plan, &mut binder)?;
    binder.tick(1)?;
    Ok(Some(plan))
}
