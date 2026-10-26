//! Validate and finish a domain plan after source expansion returns.
use super::{Binder, BoundDomain, DomainPreparation, projection};
use crate::{
    QueryError,
    external_sources::SourceNamespace,
    plan::{DomainFunction, Expr, ReadNode, ReadPlan, Relation, SelectPlan, Slot},
};
use tablecraft_sql::select::Expression;

#[inline(never)]
pub(super) fn finish(
    mut binder: Binder<'_>,
    relation: Result<Relation, QueryError>,
    owner: SourceNamespace,
    kind: DomainFunction,
    value: Option<&Expression>,
    filter: Option<&Expression>,
) -> Result<DomainPreparation, QueryError> {
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
        .map(|e| binder.expression(e, &scopes, false, 1))
        .transpose()?;
    if let Some(value) = &value {
        binder.domain_scalar(value, &relation.slots, false, 1)?;
    }
    let filter = match filter
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
            group_types: Vec::new(),
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
