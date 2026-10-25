//! Proven integral/NULL keys retain group states rather than source row groups.
mod finish;
mod state;

use super::{StreamError, StreamSummary, plan::ProgressiveArm};
use crate::{
    OutputType, QueryError,
    execution::{Environment, Runtime},
    plan::{Expr, RelationNode},
    source::ProjectedRow,
};
use state::Group;
use tablecraft_core::values::Value;

impl Runtime<'_, '_> {
    pub(super) fn progressive_group_rows<E>(
        &mut self,
        arm: &mut ProgressiveArm<'_>,
        union_types: Option<&[OutputType]>,
        summary: &mut StreamSummary,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), E>,
    ) -> Result<(), StreamError<E>> {
        self.check_table_schema(&arm.select.source.node)?;
        self.tick()?;
        let RelationNode::Table {
            name,
            columns,
            projection,
            ..
        } = &arm.select.source.node
        else {
            return Err(QueryError::Unsupported.into());
        };
        let mut lookup = Some(arm.group_lookup.take().ok_or(QueryError::Source)?);
        let policies = self.expression_policies(&arm.select.group)?;
        let mut cohort = self.progressive_key_cohort(arm.select.group.len())?;
        let mut groups: Vec<Group> = Vec::new();
        let source = self.source;
        let mut failure: Option<QueryError> = None;
        let mut consume = |input: ProjectedRow<'_>| {
            if let Some(error) = &failure {
                return Err(error.clone());
            }
            let result: Result<(), QueryError> = (|| {
                self.tick()?;
                let row = self.copy_projected_row(columns, projection, input)?;
                summary.scanned_rows = summary
                    .scanned_rows
                    .checked_add(1)
                    .ok_or(QueryError::StructureLimit)?;
                let environment = Environment {
                    aggregate_results: None,
                    row: &row,
                    collections: None,
                    outer: &[],
                    output: &[],
                    group: None,
                    group_keys: None,
                };
                if let Some(filter) = &arm.select.filter
                    && !self.eval_predicate(filter, environment)?.is_true()
                {
                    return Ok(());
                }
                let mut keys = self.allocate_expression_row(arm.select.group.len())?;
                for expression in &arm.select.group {
                    keys.push(self.repeatable_comparison_key(expression, environment)?);
                }
                if !lookup.as_ref().ok_or(QueryError::Source)?.valid(
                    &keys,
                    self.budget,
                    self.options.cancelled,
                )? {
                    // Unlike buffered general grouping, this streaming profile
                    // cannot retain an unexpected population for later regrouping.
                    return Err(QueryError::Source);
                }
                self.observe_progressive_keys(&mut cohort, &keys)?;
                let (index, fingerprint) = self.exact_key_match(
                    &mut lookup,
                    &keys,
                    &groups,
                    |group| &group.keys,
                    &policies,
                )?;
                let index = if let Some(index) = index {
                    self.merge_progressive_keys(&mut groups[index].keys, &keys)?;
                    index
                } else {
                    let index = groups.len();
                    let accumulators = self.progressive_accumulators(&arm.aggregates)?;
                    self.push(&mut groups, Group { keys, accumulators })?;
                    lookup.as_mut().ok_or(QueryError::Source)?.insert(
                        fingerprint.ok_or(QueryError::Source)?,
                        index,
                        self.budget,
                        self.options.cancelled,
                    )?;
                    index
                };
                for (expression, accumulator) in
                    arm.aggregates.iter().zip(&mut groups[index].accumulators)
                {
                    self.tick()?;
                    let Expr::Aggregate(_, argument) = expression else {
                        return Err(QueryError::Source);
                    };
                    self.push_progressive_aggregate(accumulator, argument.as_deref(), environment)?;
                }
                Ok(())
            })();
            if let Err(error) = &result {
                failure = Some(error.clone());
            }
            result
        };
        let scanned = source.scan_projected_progressive(name, projection, &mut consume);
        if let Some(error) = failure {
            return Err(StreamError::Query(error));
        }
        scanned?;
        self.tick()?;
        self.complete_progressive_keys(&mut groups, &cohort)?;
        self.finish_progressive_groups(arm, groups, union_types, summary, visitor)
    }
}
