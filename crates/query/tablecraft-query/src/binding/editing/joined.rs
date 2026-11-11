//! One explicitly selected local target occurrence in a scalar joined UPDATE.
use super::BoundAssignment;
use crate::{
    QueryError, Source,
    binding::{Binder, Parameter, resolve},
    external_sources::SourceNamespace,
    plan::{Expr, Relation, RelationNode},
};
use std::ops::Range;
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::Column;
use tablecraft_sql::{
    query::{Query, Statement},
    select::TableSource,
};

pub(crate) struct JoinedTargetSource<'a> {
    pub table: &'a str,
    pub qualifier: &'a str,
    pub columns: &'a [Column],
}

pub(crate) struct Target {
    pub leaf: usize,
    pub columns: Range<usize>,
}

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
    for relation in &update.sources {
        syntax(relation, 1, &mut binder)?;
    }
    let relation = bind_sources(&update.sources, &mut binder, 1)?;
    let mut target = None;
    find_target(&relation, &selected, &mut 0, 0, &mut target, &mut binder)?;
    let target = target.ok_or(QueryError::UnknownName)?;
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
        .is_some_and(|value| !crate::binding::mutation_scalar::admitted(value))
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

fn bind_sources(
    sources: &[TableSource],
    binder: &mut Binder<'_>,
    depth: usize,
) -> Result<Relation, QueryError> {
    binder.tick(depth)?;
    if let [source] = sources {
        return binder.relation(source, &[], depth, SourceNamespace::Root);
    }
    let middle = sources.len() / 2;
    if middle == 0 {
        return Err(QueryError::Unsupported);
    }
    let left = bind_sources(&sources[..middle], binder, depth + 1)?;
    let right = bind_sources(&sources[middle..], binder, depth + 1)?;
    let slots = binder.combine_slots(&left.slots, &right.slots)?;
    binder.charge(2 * size_of::<Relation>())?;
    Ok(Relation {
        slots,
        node: RelationNode::Cross(Box::new(left), Box::new(right)),
    })
}

fn syntax(source: &TableSource, depth: usize, binder: &mut Binder<'_>) -> Result<(), QueryError> {
    binder.tick(depth)?;
    match source {
        TableSource::Table(table) if table.database.is_none() => Ok(()),
        TableSource::Join { left, right, .. } => {
            syntax(left, depth + 1, binder)?;
            syntax(right, depth + 1, binder)
        }
        TableSource::Group {
            source,
            alias: None,
        } => syntax(source, depth + 1, binder),
        _ => Err(QueryError::Unsupported),
    }
}

fn find_target(
    relation: &Relation,
    selected: &JoinedTargetSource<'_>,
    leaf: &mut usize,
    base: usize,
    target: &mut Option<Target>,
    binder: &mut Binder<'_>,
) -> Result<(), QueryError> {
    binder.work()?;
    match &relation.node {
        RelationNode::Table {
            name,
            columns,
            multivalues,
            members,
            recordsets,
            ..
        } => {
            if !multivalues.is_empty()
                || !members.is_empty()
                || !recordsets.is_empty()
                || relation
                    .slots
                    .iter()
                    .any(|slot| !slot.available || slot.member.is_some())
            {
                return Err(QueryError::Unsupported);
            }
            let matches = relation
                .slots
                .first()
                .and_then(|slot| slot.qualifier.as_deref())
                .is_some_and(|name| name.eq_ignore_ascii_case(selected.qualifier));
            if matches {
                if !name.eq_ignore_ascii_case(selected.table) {
                    return Err(QueryError::Source);
                }
                if columns != selected.columns {
                    return Err(QueryError::SchemaChanged);
                }
                let end = base
                    .checked_add(relation.row_width())
                    .ok_or(QueryError::StructureLimit)?;
                if target
                    .replace(Target {
                        leaf: *leaf,
                        columns: base..end,
                    })
                    .is_some()
                {
                    return Err(QueryError::AmbiguousName);
                }
            }
            *leaf = leaf.checked_add(1).ok_or(QueryError::StructureLimit)?;
        }
        RelationNode::Join {
            left,
            right,
            condition,
            ..
        } => {
            if !crate::binding::mutation_scalar::admitted(condition) {
                return Err(QueryError::Unsupported);
            }
            find_target(left, selected, leaf, base, target, binder)?;
            let base = base
                .checked_add(left.row_width())
                .ok_or(QueryError::StructureLimit)?;
            find_target(right, selected, leaf, base, target, binder)?;
        }
        RelationNode::Cross(left, right) => {
            find_target(left, selected, leaf, base, target, binder)?;
            let base = base
                .checked_add(left.row_width())
                .ok_or(QueryError::StructureLimit)?;
            find_target(right, selected, leaf, base, target, binder)?;
        }
        _ => return Err(QueryError::Unsupported),
    }
    Ok(())
}
