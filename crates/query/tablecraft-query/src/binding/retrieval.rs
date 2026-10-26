mod body;
mod source;

use self::source::SelectInput;
use super::Binder;
use crate::profile::MAX_SORT_KEYS;
use crate::profile::SOURCE_SCHEMA_POLICY;
use crate::{QueryError, plan::*};
use tablecraft_sql::{
    query::{ReadBody, ReadQuery},
    select::{Expression as E, Projection, Select, SelectPredicate, SortKey},
};

impl Binder<'_> {
    pub(super) fn read(
        &mut self,
        read: &ReadQuery,
        outer: &[&[Slot]],
        depth: usize,
    ) -> Result<ReadPlan, QueryError> {
        let previous = std::mem::take(&mut self.aliases);
        let result = self.with_collection_criteria(false, |binder| {
            binder.tick(depth)?;
            if read.owner_access {
                return Err(QueryError::Unsupported);
            }
            binder.body(&read.body, &read.order_by, outer, depth + 1)
        });
        self.aliases = previous;
        result
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
            ReadBody::Select(select) => self.select(select, sort, outer, depth + 1, None),
            ReadBody::Reference(name) => self.reference_body(name, true, sort, outer, depth),
            ReadBody::Table(name) => self.reference_body(name, false, sort, outer, depth),
            ReadBody::Union { first, rest } => self.union_body(first, rest, sort, outer, depth),
        }
    }

    pub(super) fn select(
        &mut self,
        select: &Select,
        final_sort: &[SortKey],
        outer: &[&[Slot]],
        depth: usize,
        transform: Option<&E>,
    ) -> Result<ReadPlan, QueryError> {
        let previous = std::mem::take(&mut self.aliases);
        let result = self.select_input(select, outer, depth).and_then(|input| {
            self.select_with_aliases(select, final_sort, outer, depth, transform, input)
        });
        self.aliases = previous;
        result
    }

    // Source expansion finishes before entering the larger expression-planning frame.
    #[inline(never)]
    fn select_with_aliases(
        &mut self,
        select: &Select,
        final_sort: &[SortKey],
        outer: &[&[Slot]],
        depth: usize,
        transform: Option<&E>,
        input: SelectInput,
    ) -> Result<ReadPlan, QueryError> {
        let SelectInput {
            source,
            distinct,
            top,
        } = input;
        let scopes = std::iter::once(source.slots.as_slice())
            .chain(outer.iter().copied())
            .collect::<Vec<_>>();
        let filter = select
            .selection
            .as_ref()
            .map(|e| self.filter_expression(e, &scopes, depth + 1))
            .transpose()?;
        let group = select
            .group_by
            .iter()
            .map(|e| self.expression(e, &scopes, false, depth + 1))
            .collect::<Result<Vec<_>, _>>()?;
        self.charge(size_of::<Vec<ProfiledType>>() + group.len() * size_of::<ProfiledType>())?;
        let mut group_types = Vec::new();
        group_types
            .try_reserve_exact(group.len())
            .map_err(|_| QueryError::Allocation)?;
        for key in &group {
            if !self.scalar_group_key(key, depth + 1)? {
                return Err(QueryError::Unsupported);
            }
            group_types.push(self.profiled_result_type(key, &scopes, depth + 1)?);
        }
        let mut projection = Vec::new();
        let mut names = Vec::new();
        self.aliases.enabled = true;
        for item in &select.projection {
            match item {
                Projection::Wildcard(qualifier) => {
                    let mut matched = false;
                    let mut index = 0;
                    for slot in &source.slots {
                        if slot.member.is_none()
                            && qualifier.as_ref().is_none_or(|q| {
                                slot.qualifier.as_deref().is_some_and(|s| {
                                    SOURCE_SCHEMA_POLICY.names.equals(s, q.as_str())
                                })
                            })
                        {
                            if !slot.available {
                                return Err(QueryError::Unsupported);
                            }
                            matched = true;
                            projection.push(self.field(0, index, &source.slots)?);
                            names.push(slot.name.clone());
                        }
                        index += usize::from(slot.available);
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
            if projection.len() > SOURCE_SCHEMA_POLICY.max_columns {
                return Err(QueryError::StructureLimit);
            }
            let alias = match item {
                Projection::Expression { alias, .. } | Projection::Field { alias, .. } => alias,
                Projection::Wildcard(_) => &None,
            };
            if let Some(alias) = alias {
                let index = projection.len() - 1;
                self.register_projection_alias(
                    alias.as_str(),
                    index,
                    &projection[index],
                    &scopes,
                    depth + 1,
                )?;
            }
        }
        self.aliases.enabled = false;
        if self.aliases.used && transform.is_some() {
            return Err(QueryError::Unsupported);
        }
        if let Some(transform) = transform {
            projection.push(self.expression(transform, &scopes, true, depth + 1)?);
            // Internal crosstab cell slot has no resolvable output alias.
            names.push(String::new());
            if projection.len() > SOURCE_SCHEMA_POLICY.max_columns {
                return Err(QueryError::StructureLimit);
            }
        }
        if names
            .iter()
            .any(|n| n.len() > SOURCE_SCHEMA_POLICY.max_name_bytes)
        {
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
        let mut text_policies = self.policy_vector(projection.len())?;
        for expr in &projection {
            text_policies.push(self.text_policy(expr)?);
        }
        let mut output_types = self.type_vector(projection.len())?;
        let mut materialized_types = self.type_vector(projection.len())?;
        let mut declared_types = self.type_vector(projection.len())?;
        for expression in &projection {
            let types = self.profiled_result_type(expression, &scopes, depth + 1)?;
            output_types.push(types.conservative());
            materialized_types.push(types.materialized);
            declared_types.push(types.declared);
        }
        let mut declarations = self.declaration_vector(projection.len())?;
        for expression in &projection {
            declarations.push(self.result_declaration(expression, &scopes, depth + 1)?);
        }
        let output = names
            .iter()
            .zip(&text_policies)
            .zip(&output_types)
            .zip(&declarations)
            .enumerate()
            .map(|(index, (((n, policy), _), declaration))| Slot {
                output_type: ProfiledType {
                    materialized: materialized_types[index],
                    declared: declared_types[index],
                },
                declaration: *declaration,
                text_policy: *policy,
                available: true,
                member: None,
                name: n.clone(),
                qualifier: None,
            })
            .collect::<Vec<_>>();
        let mut sort = self.sort(sort, &source.slots, &output, outer, true, depth + 1)?;
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
                self.validate_group(expr, &group, depth + 1)?;
            }
        }
        let record_keys = if matches!(
            select.predicate,
            SelectPredicate::DistinctRow | SelectPredicate::DistinctRowTop { .. }
        ) {
            self.record_keys(&source, &projection, aggregate, depth + 1)?
        } else {
            Vec::new()
        };
        let (filter, parent) = if transform.is_none() {
            self.parent_criteria(
                &source,
                projection
                    .iter()
                    .chain(&group)
                    .chain(having.iter())
                    .chain(sort.iter().map(|(expression, _)| expression)),
                filter,
                depth + 1,
            )?
        } else {
            (filter, None)
        };
        if distinct || !record_keys.is_empty() {
            self.projected_sort(&mut sort, &projection, &text_policies, depth + 1)?;
        }
        // DISTINCT sorts must identify a projected value, not an arbitrary member.
        if (distinct || !record_keys.is_empty())
            && sort.iter().any(|(e, _)| !matches!(e, Expr::Output(_, _)))
        {
            return Err(QueryError::Unsupported);
        }
        Ok(ReadPlan {
            owner: self.owner,
            declarations,
            output_types,
            materialized_types,
            declared_types,
            text_policies,
            names,
            node: ReadNode::Select(Box::new(SelectPlan {
                source,
                projection,
                filter,
                group,
                group_types,
                having,
                aggregate,
                record_keys,
                parent,
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
        if keys.len() > MAX_SORT_KEYS {
            return Err(QueryError::StructureLimit);
        }
        let mut result = Vec::new();
        for key in keys {
            let expr = match &key.expression {
                E::Integer(n) if !output.is_empty() => {
                    if *n < 1 || *n as usize > output.len() {
                        return Err(QueryError::UnknownName);
                    }
                    Expr::Output(*n as usize - 1, output[*n as usize - 1].text_policy)
                }
                E::Field(f) if f.qualifier.is_none() && !output.is_empty() => {
                    if let Some(i) = super::resolve(f, output)? {
                        Expr::Output(i, output[i].text_policy)
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
pub(super) fn has_aggregate(expr: &Expr) -> bool {
    match expr {
        Expr::Aggregate(..) => true,
        Expr::Unary(_, e)
        | Expr::IsNull(e, _)
        | Expr::CollectionEmpty(e, _)
        | Expr::DateCall(_, e)
        | Expr::Convert(_, e)
        | Expr::CVar(e)
        | Expr::CvErr(e)
        | Expr::Eval(e)
        | Expr::GuidFromString(e)
        | Expr::ErrorMessage(e)
        | Expr::ErrorMessageString(e)
        | Expr::StringFromGuid(e)
        | Expr::Inspect(_, e)
        | Expr::ValueInspect(_, e)
        | Expr::DateConvert(_, e) => has_aggregate(e),
        Expr::Binary(_, a, b) | Expr::ArrayBound(_, a, b) | Expr::Like(a, b, _) => {
            has_aggregate(a) || has_aggregate(b)
        }
        Expr::Between(a, b, c, _) | Expr::IIf(a, b, c) | Expr::DateAdd(a, b, c) => {
            has_aggregate(a) || has_aggregate(b) || has_aggregate(c)
        }
        Expr::DateDiff(a, b, c, options) => [a, b, c]
            .into_iter()
            .chain(options.iter().flatten())
            .any(|expr| has_aggregate(expr)),
        Expr::DatePart(a, b, options) => [a, b]
            .into_iter()
            .chain(options.iter().flatten())
            .any(|expr| has_aggregate(expr)),
        Expr::DomainCall(_, arguments)
        | Expr::TextCall(_, arguments)
        | Expr::FinancialCall(_, arguments)
        | Expr::Array(arguments)
        | Expr::CashFlowCall(_, arguments, _)
        | Expr::MathCall(_, arguments)
        | Expr::ConstructorCall(_, arguments)
        | Expr::ConditionalCall(_, arguments) => arguments.iter().any(has_aggregate),
        Expr::Nz(value, replacement) | Expr::Weekday(value, replacement) => {
            has_aggregate(value) || replacement.as_ref().is_some_and(|expr| has_aggregate(expr))
        }
        Expr::InList(e, list, _) => has_aggregate(e) || list.iter().any(has_aggregate),
        Expr::InQuery(e, ..) | Expr::Quantified(e, ..) => has_aggregate(e),
        _ => false,
    }
}
