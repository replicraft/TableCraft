use super::{Binder, relation::combine_slots};
use crate::{QueryError, plan::*};
use tablecraft_sql::{
    query::{ReadBody, ReadQuery},
    select::{Expression as E, Projection, Select, SelectPredicate, SortKey, Table, TableSource},
};

impl Binder<'_> {
    pub(super) fn read(
        &mut self,
        read: &ReadQuery,
        outer: &[&[Slot]],
        depth: usize,
    ) -> Result<ReadPlan, QueryError> {
        self.tick(depth)?;
        if read.owner_access {
            return Err(QueryError::Unsupported);
        }
        self.body(&read.body, &read.order_by, outer, depth + 1)
    }
    fn body(
        &mut self,
        body: &ReadBody,
        sort: &[SortKey],
        outer: &[&[Slot]],
        depth: usize,
    ) -> Result<ReadPlan, QueryError> {
        self.tick(depth)?;
        match body {
            ReadBody::Select(select) => self.select(select, sort, outer, depth + 1),
            ReadBody::Reference(_) => Err(QueryError::Unsupported),
            ReadBody::Table(name) => self.select(
                &Select {
                    predicate: SelectPredicate::All,
                    projection: vec![Projection::Wildcard(None)],
                    from: vec![TableSource::Table(Table {
                        name: name.clone(),
                        alias: None,
                    })],
                    group_by: vec![],
                    having: None,
                    into: None,
                    into_external: None,
                    external: None,
                    owner_access: false,
                    selection: None,
                    order_by: vec![],
                },
                sort,
                outer,
                depth + 1,
            ),
            ReadBody::Union { first, rest } => {
                if rest.len() > 256 {
                    return Err(QueryError::StructureLimit);
                }
                let first = self.body(first, &[], outer, depth + 1)?;
                let mut arms = Vec::new();
                for arm in rest {
                    let plan = self.body(&arm.body, &[], outer, depth + 1)?;
                    if plan.names.len() != first.names.len() {
                        return Err(QueryError::Width);
                    }
                    arms.push((arm.all, plan));
                }
                let slots = first
                    .names
                    .iter()
                    .map(|n| Slot {
                        name: n.clone(),
                        qualifier: None,
                    })
                    .collect::<Vec<_>>();
                let order = self.sort(sort, &[], &slots, outer, false, depth + 1)?;
                Ok(ReadPlan {
                    names: first.names.clone(),
                    node: ReadNode::Union {
                        first: Box::new(first),
                        rest: arms,
                    },
                    sort: order,
                    distinct: false,
                    top: None,
                })
            }
        }
    }
    fn select(
        &mut self,
        select: &Select,
        final_sort: &[SortKey],
        outer: &[&[Slot]],
        depth: usize,
    ) -> Result<ReadPlan, QueryError> {
        self.tick(depth)?;
        if select.into.is_some()
            || select.into_external.is_some()
            || select.external.is_some()
            || select.owner_access
        {
            return Err(QueryError::Unsupported);
        }
        let (distinct, top) = match select.predicate {
            SelectPredicate::All => (false, None),
            SelectPredicate::Distinct => (true, None),
            SelectPredicate::Top { count, percent } => (false, Some((count, percent))),
            SelectPredicate::DistinctTop { count, percent } => (true, Some((count, percent))),
            _ => return Err(QueryError::Unsupported),
        };
        if top.is_some_and(|(n, p)| n == 0 || (p && n > 100)) {
            return Err(QueryError::Unsupported);
        }
        if select.from.len() > 16
            || select.projection.is_empty()
            || select.projection.len() > 256
            || select.group_by.len() > 10
        {
            return Err(QueryError::StructureLimit);
        }
        let mut source = Relation {
            slots: vec![],
            node: RelationNode::Unit,
        };
        for item in &select.from {
            let right = self.relation(item, outer, depth + 1)?;
            let slots = combine_slots(&source.slots, &right.slots)?;
            source = Relation {
                slots,
                node: RelationNode::Cross(Box::new(source), Box::new(right)),
            };
        }
        let scopes = std::iter::once(source.slots.as_slice())
            .chain(outer.iter().copied())
            .collect::<Vec<_>>();
        let filter = select
            .selection
            .as_ref()
            .map(|e| self.expression(e, &scopes, false, depth + 1))
            .transpose()?;
        let group = select
            .group_by
            .iter()
            .map(|e| self.expression(e, &scopes, false, depth + 1))
            .collect::<Result<Vec<_>, _>>()?;
        // Initial grouping profile: bound input fields, not arbitrary expression equivalence.
        if group
            .iter()
            .any(|e| !matches!(e, Expr::Field { depth: 0, .. }))
        {
            return Err(QueryError::Unsupported);
        }
        let mut projection = Vec::new();
        let mut names = Vec::new();
        for item in &select.projection {
            match item {
                Projection::Wildcard(qualifier) => {
                    let mut matched = false;
                    for (index, slot) in source.slots.iter().enumerate() {
                        if qualifier.as_ref().is_none_or(|q| {
                            slot.qualifier
                                .as_deref()
                                .is_some_and(|s| s.eq_ignore_ascii_case(q.as_str()))
                        }) {
                            matched = true;
                            projection.push(Expr::Field { depth: 0, index });
                            names.push(slot.name.clone());
                        }
                    }
                    if !matched {
                        return Err(QueryError::UnknownName);
                    }
                }
                Projection::Expression { expression, alias } => {
                    projection.push(self.expression(expression, &scopes, true, depth + 1)?);
                    names.push(
                        alias.as_ref().map_or_else(
                            || format!("Expr{}", names.len() + 1),
                            |n| n.as_str().into(),
                        ),
                    );
                }
                Projection::Field { field, alias } => {
                    projection.push(self.expression(
                        &E::Field(field.clone()),
                        &scopes,
                        true,
                        depth + 1,
                    )?);
                    names.push(alias.as_ref().unwrap_or(&field.name).as_str().into());
                }
            }
            if projection.len() > 256 {
                return Err(QueryError::StructureLimit);
            }
        }
        if names.iter().any(|n| n.len() > 255) {
            return Err(QueryError::StructureLimit);
        }
        self.charge(
            names
                .iter()
                .map(|n| 2 * n.len() + std::mem::size_of::<Slot>())
                .sum(),
        )?;
        let having = select
            .having
            .as_ref()
            .map(|e| self.expression(e, &scopes, true, depth + 1))
            .transpose()?;
        if !select.order_by.is_empty() && !final_sort.is_empty() {
            return Err(QueryError::Unsupported);
        }
        let sort = if final_sort.is_empty() {
            &select.order_by
        } else {
            final_sort
        };
        let output = names
            .iter()
            .map(|n| Slot {
                name: n.clone(),
                qualifier: None,
            })
            .collect::<Vec<_>>();
        let sort = self.sort(sort, &source.slots, &output, outer, true, depth + 1)?;
        let aggregate = !group.is_empty()
            || projection
                .iter()
                .chain(having.iter())
                .chain(sort.iter().map(|(e, _)| e))
                .any(has_aggregate);
        if having.is_some() && !aggregate {
            return Err(QueryError::Grouping);
        }
        if aggregate {
            for expr in projection
                .iter()
                .chain(having.iter())
                .chain(sort.iter().map(|(e, _)| e))
            {
                validate_group(expr, &group)?;
            }
        }
        // DISTINCT sorting is restricted to output columns to avoid undefined representative keys.
        if distinct && sort.iter().any(|(e, _)| !matches!(e, Expr::Output(_))) {
            return Err(QueryError::Unsupported);
        }
        Ok(ReadPlan {
            names,
            node: ReadNode::Select(Box::new(SelectPlan {
                source,
                projection,
                filter,
                group,
                having,
                aggregate,
            })),
            sort,
            distinct,
            top,
        })
    }
    fn sort(
        &mut self,
        keys: &[SortKey],
        input: &[Slot],
        output: &[Slot],
        outer: &[&[Slot]],
        aggregates: bool,
        depth: usize,
    ) -> Result<Vec<(Expr, bool)>, QueryError> {
        if keys.len() > 10 {
            return Err(QueryError::StructureLimit);
        }
        let mut result = Vec::new();
        for key in keys {
            let expr = match &key.expression {
                E::Integer(n) if !output.is_empty() => {
                    if *n < 1 || *n as usize > output.len() {
                        return Err(QueryError::UnknownName);
                    }
                    Expr::Output(*n as usize - 1)
                }
                E::Field(f) if f.qualifier.is_none() && !output.is_empty() => {
                    if let Some(i) = super::resolve(f, output)? {
                        Expr::Output(i)
                    } else {
                        let scopes = std::iter::once(input)
                            .chain(outer.iter().copied())
                            .collect::<Vec<_>>();
                        self.expression(&key.expression, &scopes, aggregates, depth + 1)?
                    }
                }
                _ => {
                    let scopes = std::iter::once(input)
                        .chain(outer.iter().copied())
                        .collect::<Vec<_>>();
                    self.expression(&key.expression, &scopes, aggregates, depth + 1)?
                }
            };
            result.push((expr, key.descending));
        }
        Ok(result)
    }
}
fn has_aggregate(expr: &Expr) -> bool {
    match expr {
        Expr::Aggregate(..) => true,
        Expr::Unary(_, e) | Expr::IsNull(e, _) => has_aggregate(e),
        Expr::Binary(_, a, b) => has_aggregate(a) || has_aggregate(b),
        Expr::Between(a, b, c, _) => has_aggregate(a) || has_aggregate(b) || has_aggregate(c),
        Expr::InList(e, list, _) => has_aggregate(e) || list.iter().any(has_aggregate),
        Expr::InQuery(e, ..) | Expr::Quantified(e, ..) => has_aggregate(e),
        _ => false,
    }
}
fn validate_group(expr: &Expr, keys: &[Expr]) -> Result<(), QueryError> {
    match expr {
        Expr::Field { depth: 0, index }
            if !keys
                .iter()
                .any(|k| matches!(k,Expr::Field {depth:0,index:i} if i==index)) =>
        {
            return Err(QueryError::Grouping);
        }
        Expr::Unary(_, e) | Expr::IsNull(e, _) => validate_group(e, keys)?,
        Expr::Binary(_, a, b) => {
            validate_group(a, keys)?;
            validate_group(b, keys)?;
        }
        Expr::Between(a, b, c, _) => {
            validate_group(a, keys)?;
            validate_group(b, keys)?;
            validate_group(c, keys)?;
        }
        Expr::InList(e, list, _) => {
            validate_group(e, keys)?;
            for e in list {
                validate_group(e, keys)?;
            }
        }
        Expr::Subquery(_) | Expr::Exists(_) | Expr::InQuery(..) | Expr::Quantified(..) => {
            return Err(QueryError::Unsupported);
        }
        _ => {}
    }
    Ok(())
}
