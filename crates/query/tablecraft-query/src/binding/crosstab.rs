//! Metadata-only TRANSFORM binding; execution owns dynamic heading discovery.
mod constants;
mod schema;
use super::{Binder, projection, retrieval::has_aggregate};
use crate::{
    OutputType, QueryError,
    plan::{CrosstabPlan, Expr, ReadNode, ReadPlan},
    profile::SOURCE_SCHEMA_POLICY,
};
use tablecraft_sql::{query::Crosstab, select::SelectPredicate};

impl Binder<'_> {
    pub(super) fn crosstab(&mut self, cross: &Crosstab) -> Result<CrosstabPlan, QueryError> {
        self.tick(1)?;
        if !matches!(
            cross.select.predicate,
            SelectPredicate::All | SelectPredicate::Top { .. }
        ) || cross.select.group_by.is_empty()
        {
            return Err(QueryError::Unsupported);
        }
        let mut read = self.select(&cross.select, &[], &[], 2, Some(&cross.transform))?;
        let ReadNode::Select(bound) = &mut read.node else {
            return Err(QueryError::Unsupported);
        };
        if !bound.projection.last().is_some_and(has_aggregate) {
            return Err(QueryError::Grouping);
        }
        let row_width = bound.projection.len() - 1;
        if read
            .sort
            .iter()
            .any(|(expr, _)| matches!(expr,Expr::Output(index, _) if *index >= row_width))
        {
            return Err(QueryError::Unsupported);
        }
        let pivot = self.expression(&cross.pivot, &[&bound.source.slots], false, 2)?;
        if !self.same_group_expression(&pivot, &pivot, 2)? {
            return Err(QueryError::Unsupported);
        }
        // Temporarily collect the pivot as a key so the shared pruner retains
        // its fields and rewrites every slot consistently. It does not become a
        // row grouping key in the executable plan.
        bound.group.push(pivot);
        // Pivot-only collections are displayed too. Classify hidden WHERE
        // members only after the complete expansion domain is known.
        let filter = bound.filter.take();
        let (filter, parent) = self.parent_criteria(
            &bound.source,
            bound
                .projection
                .iter()
                .chain(&bound.group)
                .chain(bound.having.iter())
                .chain(read.sort.iter().map(|(expression, _)| expression)),
            filter,
            2,
        )?;
        bound.filter = filter;
        bound.parent = parent;
        projection::prune(&mut read, self)?;
        let ReadNode::Select(bound) = &mut read.node else {
            return Err(QueryError::Unsupported);
        };
        let pivot = bound.group.pop().ok_or(QueryError::Grouping)?;
        let headings = if let Some(headings) = &cross.headings {
            if headings.is_empty() || headings.len() > SOURCE_SCHEMA_POLICY.max_columns - row_width
            {
                return Err(QueryError::StructureLimit);
            }
            let mut result = Vec::new();
            self.charge(headings.len() * size_of::<Expr>())?;
            result
                .try_reserve_exact(headings.len())
                .map_err(|_| QueryError::Allocation)?;
            for heading in headings {
                let expr = self.expression(heading, &[], false, 2)?;
                if !self.same_group_expression(&expr, &expr, 2)? {
                    return Err(QueryError::Unsupported);
                }
                result.push(expr);
            }
            Some(result)
        } else {
            None
        };
        let text_policies = if let Some(headings) = &headings {
            let mut policies = self.policy_vector(row_width + headings.len())?;
            policies.extend_from_slice(&read.text_policies[..row_width]);
            policies.extend(std::iter::repeat_n(
                *read.text_policies.last().ok_or(QueryError::Grouping)?,
                headings.len(),
            ));
            Some(policies)
        } else {
            None
        };
        let output_types = headings
            .as_ref()
            .map(|headings| {
                self.crosstab_heading_types(&read.output_types, row_width, headings.len())
            })
            .transpose()?;
        let materialized_types = headings
            .as_ref()
            .map(|headings| {
                self.crosstab_heading_types(&read.materialized_types, row_width, headings.len())
            })
            .transpose()?;
        let declared_types = headings
            .as_ref()
            .map(|headings| {
                self.crosstab_heading_types(&read.declared_types, row_width, headings.len())
            })
            .transpose()?;
        self.charge(size_of::<ReadPlan>())?;
        Ok(CrosstabPlan {
            output_types,
            materialized_types,
            declared_types,
            materialized: None,
            text_policies,
            read: Box::new(read),
            pivot,
            headings,
        })
    }
    fn crosstab_heading_types(
        &mut self,
        source: &[OutputType],
        row_width: usize,
        headings: usize,
    ) -> Result<Vec<OutputType>, QueryError> {
        let mut types = self.type_vector(row_width + headings)?;
        types.extend_from_slice(&source[..row_width]);
        let cell = source.last().ok_or(QueryError::Grouping)?.with_null();
        types.extend(std::iter::repeat_n(cell, headings));
        Ok(types)
    }
}
