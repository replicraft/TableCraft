//! Proven integral/NULL keys retain group states rather than input row groups.
mod finish;
mod state;

use super::{StreamError, StreamSummary, plan::ProgressiveArm};
use crate::{
    OutputType, QueryError,
    execution::{Environment, Runtime, cell::CellValue},
    external_sources::ExternalBinding,
    plan::Expr,
};
use state::Group;
pub(super) use state::KeyPolicy;
use tablecraft_core::values::Value;

impl Runtime<'_, '_> {
    pub(super) fn progressive_group_rows<E>(
        &mut self,
        arm: &mut ProgressiveArm<'_>,
        union_types: Option<&[OutputType]>,
        bindings: &[ExternalBinding],
        summary: &mut StreamSummary,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), E>,
    ) -> Result<(), StreamError<E>> {
        let source = self.progressive_source(&arm.input, bindings)?;
        let read = arm.read;
        let select = arm.select;
        let expressions = &arm.aggregates;
        let mut lookup = Some(arm.group_lookup.take().ok_or(QueryError::Source)?);
        let (policies, mut cohort) = self.with_progressive_owner(read.owner, |runtime| {
            Ok::<_, QueryError>((
                runtime.expression_policies(&select.group)?,
                runtime.progressive_key_cohort(select.group.len())?,
            ))
        })?;
        let mut groups: Vec<Group> = Vec::new();
        let mut consume = |runtime: &mut Self, row: Vec<CellValue>, _: &mut StreamSummary| {
            let environment = Environment {
                aggregate_results: None,
                row: &row,
                collections: None,
                outer: &[],
                output: &[],
                group: None,
                group_keys: None,
            };
            if let Some(filter) = &select.filter
                && !runtime.eval_predicate(filter, environment)?.is_true()
            {
                return Ok(());
            }
            let mut keys = runtime.allocate_expression_row(select.group.len())?;
            for expression in &select.group {
                keys.push(runtime.repeatable_comparison_key(expression, environment)?);
            }
            if !lookup.as_ref().ok_or(QueryError::Source)?.valid(
                &keys,
                runtime.budget,
                runtime.options.cancelled,
            )? {
                return Err(QueryError::Source.into());
            }
            runtime.observe_progressive_keys(&mut cohort, &keys)?;
            let (index, fingerprint) = runtime.exact_key_match(
                &mut lookup,
                &keys,
                &groups,
                |group| &group.keys,
                &policies,
            )?;
            let index = if let Some(index) = index {
                runtime.merge_progressive_keys(&mut groups[index].keys, &keys)?;
                index
            } else {
                let index = groups.len();
                let accumulators = runtime.progressive_accumulators(expressions)?;
                runtime.push(&mut groups, Group { keys, accumulators })?;
                lookup.as_mut().ok_or(QueryError::Source)?.insert(
                    fingerprint.ok_or(QueryError::Source)?,
                    index,
                    runtime.budget,
                    runtime.options.cancelled,
                )?;
                index
            };
            for (expression, accumulator) in expressions.iter().zip(&mut groups[index].accumulators)
            {
                runtime.tick()?;
                let Expr::Aggregate(_, argument) = expression else {
                    return Err(QueryError::Source.into());
                };
                runtime.push_progressive_aggregate(
                    accumulator,
                    argument.as_deref(),
                    environment,
                )?;
            }
            Ok(())
        };
        self.scan_progressive_input(source, &mut arm.input, read.owner, summary, &mut consume)?;
        self.with_progressive_owner(read.owner, |runtime| {
            runtime.complete_progressive_keys(&mut groups, &cohort)?;
            runtime.finish_progressive_groups(arm, groups, union_types, summary, visitor)
        })
    }
}
