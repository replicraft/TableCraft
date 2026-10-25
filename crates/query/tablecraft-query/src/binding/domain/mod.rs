//! Runtime domain preparation uses the selected Source and shared operation budget.
mod scalar;
mod text;
use super::{Binder, projection};
use crate::{
    QueryError, Source,
    discovery::Materialized,
    plan::{DomainFunction, Expr, ReadNode, ReadPlan, SelectPlan, Slot},
};
use tablecraft_core::limits::ResourceBudget;
use tablecraft_expressions::functions::dates::parsing::ReferenceYear;

pub(crate) struct DomainContext<'a> {
    pub owner: crate::external_sources::SourceNamespace,
    pub external_sources: &'a [crate::external_sources::ExternalBinding],
    pub source: &'a dyn Source,
    pub materialized: &'a [Materialized<'a>],
    pub budget: &'a mut ResourceBudget,
    pub discover: bool,
    pub reference_year: Option<ReferenceYear>,
    pub declared_arithmetic: bool,
    pub cancelled: Option<&'a dyn Fn() -> bool>,
}

// Inner Err requests explicit execution staging; the outer Err is an actual
// binding failure. None retains the existing missing-criteria-field result.
pub(crate) struct BoundDomain {
    pub read: ReadPlan,
    pub external_sources: Vec<crate::external_sources::ExternalBinding>,
}

type DomainPreparation = Result<Option<BoundDomain>, crate::named_identity::SchemaRequest>;

pub(crate) fn bind_domain(
    context: DomainContext<'_>,
    kind: DomainFunction,
    expression: &str,
    domain: &str,
    criteria: Option<&str>,
) -> Result<DomainPreparation, QueryError> {
    let DomainContext {
        owner,
        external_sources,
        source,
        materialized,
        budget,
        cancelled,
        discover,
        reference_year,
        declared_arithmetic,
    } = context;
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
        external_sources: Vec::new(),
        external_providers: Vec::new(),
        materialized,
        discover,
        reference_year,
        declared_arithmetic,
        pending_schema: None,
        budget: Some(budget),
        cancelled,
        parameters: Vec::new(),
        parameter_scope: Vec::new(),
        retrieval: true,
        collection_criteria: false,
        active_queries: Vec::new(),
        owner,
        nodes: 0,
        policy_visits: 0,
        bytes: 0,
    };
    binder.seed_external_sources(external_sources)?;
    let relation = binder.relation(
        syntax.from.first().ok_or(QueryError::Unsupported)?,
        &[],
        1,
        owner,
    );
    // Declarations observed before a schema request must fail before discovery.
    if !binder.parameters.is_empty() {
        return Err(QueryError::Parameter);
    }
    let relation = match relation {
        Ok(relation) => relation,
        Err(QueryError::Unsupported) if binder.pending_schema.is_some() => {
            return Ok(Err(crate::named_identity::SchemaRequest {
                identity: binder
                    .pending_schema
                    .take()
                    .ok_or(QueryError::Unsupported)?,
                external_sources: binder.external_sources,
            }));
        }
        Err(error) => return Err(error),
    };
    if !binder.parameters.is_empty() {
        return Err(QueryError::Parameter);
    }
    // Domain values remain scalar. Criteria can additionally test named
    // collection presence without turning parent records into child rows.
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
        .map(|e| binder.filter_expression(e, &scopes, 1))
        .transpose()
    {
        Ok(value) => value,
        Err(QueryError::UnknownName) => return Ok(Ok(None)),
        Err(error) => return Err(error),
    };
    if let Some(filter) = &filter {
        binder.domain_scalar(filter, &relation.slots, true, 1)?;
    }
    if !binder.parameters.is_empty() {
        return Err(QueryError::Parameter);
    }
    let (filter, parent) = binder.parent_criteria(&relation, value.iter(), filter, 1)?;
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
    let output_types = binder.unknown_types(1)?;
    let materialized_types = binder.unknown_types(1)?;
    let declared_types = binder.unknown_types(1)?;
    let declarations = binder.unknown_declarations(1)?;
    let mut plan = ReadPlan {
        owner,
        declarations,
        output_types,
        materialized_types,
        declared_types,
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
            parent,
        })),
        sort: Vec::new(),
        distinct: false,
        top: None,
    };
    projection::prune(&mut plan, &mut binder)?;
    binder.tick(1)?;
    Ok(Ok(Some(BoundDomain {
        read: plan,
        external_sources: binder.external_sources,
    })))
}
