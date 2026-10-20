//! Prune source payloads after complete binding; retain all expression semantics.
use super::Binder;
use crate::{
    QueryError,
    plan::{ReadNode, ReadPlan},
    profile::{MAX_BIND_DEPTH, MAX_BIND_NODES},
};

mod collect;
mod parent;
mod relation;
mod rewrite;

type FieldMap = Vec<Option<usize>>;
// Collection and rewrite visits are bounded once per enclosing query scope.
const MAX_PROJECTION_VISITS: usize = MAX_BIND_NODES * (MAX_BIND_DEPTH + 1) * 2;

pub(super) fn prune(plan: &mut ReadPlan, binder: &mut Binder<'_>) -> Result<(), QueryError> {
    Pruner { binder, visits: 0 }.read(plan, &[])
}
struct Pruner<'a, 's> {
    binder: &'a mut Binder<'s>,
    visits: usize,
}
impl Pruner<'_, '_> {
    fn tick(&mut self) -> Result<(), QueryError> {
        self.visits += 1;
        if self.visits > MAX_PROJECTION_VISITS {
            return Err(QueryError::StructureLimit);
        }
        Ok(())
    }
    fn vector<T>(&mut self, width: usize) -> Result<Vec<T>, QueryError> {
        self.binder.charge(
            width
                .checked_mul(size_of::<T>())
                .ok_or(QueryError::StructureLimit)?,
        )?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(width)
            .map_err(|_| QueryError::Allocation)?;
        Ok(result)
    }
    fn mapping(&mut self, needed: &[bool]) -> Result<FieldMap, QueryError> {
        let mut map = self.vector(needed.len())?;
        let mut next = 0;
        for used in needed {
            map.push(if *used {
                let position = next;
                next += 1;
                Some(position)
            } else {
                None
            });
        }
        Ok(map)
    }
    fn read(&mut self, plan: &mut ReadPlan, outer: &[&[Option<usize>]]) -> Result<(), QueryError> {
        self.tick()?;
        match &mut plan.node {
            ReadNode::Select(select) => {
                let mut needed = self.vector(select.source.row_width())?;
                needed.resize(select.source.row_width(), false);
                self.local_relation(&select.source, 0, &mut needed)?;
                for expr in select
                    .projection
                    .iter()
                    .chain(&select.record_keys)
                    .chain(select.filter.iter())
                    .chain(&select.group)
                    .chain(select.having.iter())
                {
                    self.collect_expr(expr, 0, 0, &mut needed)?;
                }
                for (expr, _) in &plan.sort {
                    self.collect_expr(expr, 0, 0, &mut needed)?;
                }
                if let Some(parent) = &select.parent {
                    self.collect_parent(&parent.predicate, 0, 0, &mut needed)?;
                }
                if select.source.slots.iter().any(|slot| slot.member) {
                    let mut displayed = self.vector(select.source.row_width())?;
                    displayed.resize(select.source.row_width(), false);
                    for expr in select.projection.iter().chain(&select.group) {
                        self.collect_expr(expr, 0, 0, &mut displayed)?;
                    }
                    let mut index = 0;
                    for slot in &select.source.slots {
                        if slot.available {
                            if slot.member && needed[index] && !displayed[index] {
                                return Err(QueryError::Unsupported);
                            }
                            index += 1;
                        }
                    }
                }
                let map = self.relation(&mut select.source, &needed, outer)?;
                let mut scopes = self.vector(outer.len() + 1)?;
                scopes.push(map.as_slice());
                scopes.extend_from_slice(outer);
                for expr in select
                    .projection
                    .iter_mut()
                    .chain(&mut select.record_keys)
                    .chain(select.filter.iter_mut())
                    .chain(&mut select.group)
                    .chain(select.having.iter_mut())
                {
                    self.expression(expr, &scopes)?;
                }
                if let Some(parent) = &mut select.parent {
                    self.rewrite_parent(&mut parent.predicate, &scopes)?;
                }
                for (expr, _) in &mut plan.sort {
                    self.expression(expr, &scopes)?;
                }
            }
            ReadNode::Union { first, rest } => {
                self.read(first, outer)?;
                for (_, arm) in rest {
                    self.read(arm, outer)?;
                }
                let mut identity = self.vector(plan.names.len())?;
                identity.extend((0..plan.names.len()).map(Some));
                let mut scopes = self.vector(outer.len() + 1)?;
                scopes.push(identity.as_slice());
                scopes.extend_from_slice(outer);
                for (expr, _) in &mut plan.sort {
                    self.expression(expr, &scopes)?;
                }
            }
        }
        Ok(())
    }
}

// Reuse the bounded field walker; provenance follows relation ranges, not aliases.
pub(super) fn referenced_fields(
    expressions: &[crate::plan::Expr],
    width: usize,
    binder: &mut Binder<'_>,
) -> Result<Vec<bool>, QueryError> {
    let mut pruner = Pruner { binder, visits: 0 };
    let mut used = pruner.vector(width)?;
    used.resize(width, false);
    for expr in expressions {
        pruner.collect_expr(expr, 0, 0, &mut used)?;
    }
    Ok(used)
}
