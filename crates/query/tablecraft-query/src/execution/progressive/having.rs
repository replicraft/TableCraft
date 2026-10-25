//! HAVING gates every selected aggregate operation and final projection.
mod finish;
mod state;

use super::{StreamError, StreamSummary, plan::ProgressiveArm};
use crate::{
    OutputType, QueryError,
    execution::{Environment, Runtime, cell::CellValue},
    external_sources::ExternalBinding,
    plan::{Aggregate, Expr},
};
use state::{HavingGroup, HavingPopulation};
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;

impl Runtime<'_, '_> {
    pub(super) fn progressive_having_rows<E>(
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
        let having = select.having.as_ref().ok_or(QueryError::Source)?;
        let expressions = self.progressive_having_aggregates(having)?;
        let mut count_only = true;
        for expression in &arm.aggregates {
            self.tick()?;
            if !matches!(expression, Expr::Aggregate(Aggregate::Count, None)) {
                count_only = false;
            }
        }
        let mut lookup = arm.group_lookup.take();
        let (policies, mut cohort, mut groups) =
            self.with_progressive_owner(read.owner, |runtime| {
                let policies = runtime.expression_policies(&select.group)?;
                let cohort = runtime.progressive_key_cohort(select.group.len())?;
                let mut groups = Vec::new();
                if select.group.is_empty() {
                    let group = runtime.having_group(Vec::new(), &expressions)?;
                    runtime.push(&mut groups, group)?;
                }
                Ok::<_, QueryError>((policies, cohort, groups))
            })?;
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
            let index = if select.group.is_empty() {
                0
            } else {
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
                if let Some(index) = index {
                    runtime.merge_progressive_keys(&mut groups[index].keys, &keys)?;
                    index
                } else {
                    let index = groups.len();
                    let group = runtime.having_group(keys, &expressions)?;
                    runtime.push(&mut groups, group)?;
                    lookup.as_mut().ok_or(QueryError::Source)?.insert(
                        fingerprint.ok_or(QueryError::Source)?,
                        index,
                        runtime.budget,
                        runtime.options.cancelled,
                    )?;
                    index
                }
            };
            let group: &mut HavingGroup = groups.get_mut(index).ok_or(QueryError::Source)?;
            for (expression, accumulator) in expressions.iter().zip(&mut group.accumulators) {
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
            runtime.tick()?;
            group.count = group
                .count
                .checked_add(1)
                .ok_or(QueryError::Evaluation(EvalError::Overflow))?;
            if !count_only {
                // Move the already paid retained row. No selected operand is
                // evaluated until its group's HAVING predicate has passed.
                runtime.retain_having_row(&mut group.rows, row)?;
            }
            Ok(())
        };
        self.scan_progressive_input(source, &mut arm.input, read.owner, summary, &mut consume)?;
        self.with_progressive_owner(read.owner, |runtime| {
            for group in &mut groups {
                runtime.complete_progressive_key_cells(&mut group.keys, &cohort)?;
            }
            runtime.finish_progressive_having(
                arm,
                HavingPopulation {
                    expressions,
                    groups,
                    count_only,
                },
                union_types,
                summary,
                visitor,
            )
        })
    }
}
