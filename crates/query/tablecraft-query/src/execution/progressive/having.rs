//! HAVING gates every selected aggregate operation and final projection.
mod finish;
mod state;

use super::{
    StreamError, StreamSummary,
    plan::ProgressiveArm,
    retained::{RetainedReceiver, StageOutput},
};
use crate::{
    OutputType, QueryError,
    execution::{Environment, Runtime, cell::CellValue, exact_keys::ExactKeys},
    external_sources::ExternalBinding,
    plan::{Aggregate, Expr},
};
pub(super) use state::{DeferredGroup, HavingInput, HavingPopulation};
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
        let output = StageOutput {
            read: arm.read,
            select: arm.select,
            aggregates: &arm.aggregates,
        };
        let (expressions, count_only) = self.having_operations(output)?;
        let lookup = arm.group_lookup.take();
        let mut state = self.with_progressive_owner(output.read.owner, |runtime| {
            runtime.having_input(output, lookup, expressions, count_only)
        })?;
        let mut consume = |runtime: &mut Self, row: Vec<CellValue>, _: &mut StreamSummary| {
            runtime
                .push_having_input(output, &mut state, row)
                .map_err(Into::into)
        };
        self.scan_progressive_input(
            source,
            &mut arm.input,
            output.read.owner,
            summary,
            &mut consume,
        )?;
        self.with_progressive_owner(output.read.owner, |runtime| {
            runtime.finish_having_input(output, state, &mut |runtime, cells| {
                runtime.progressive_publish_read(output.read, cells, union_types, summary, visitor)
            })
        })
    }

    pub(super) fn having_operations<'p>(
        &mut self,
        output: StageOutput<'p, '_>,
    ) -> Result<(Vec<&'p Expr>, bool), QueryError> {
        let having = output.select.having.as_ref().ok_or(QueryError::Source)?;
        let expressions = self.progressive_having_aggregates(having)?;
        let mut count_only = true;
        for expression in output.aggregates {
            self.tick()?;
            if !matches!(expression, Expr::Aggregate(Aggregate::Count, None)) {
                count_only = false;
            }
        }
        Ok((expressions, count_only))
    }

    pub(super) fn having_input<'p>(
        &mut self,
        output: StageOutput<'p, '_>,
        lookup: Option<ExactKeys>,
        expressions: Vec<&'p Expr>,
        count_only: bool,
    ) -> Result<HavingInput<'p>, QueryError> {
        let policies = self.expression_policies(&output.select.group)?;
        let cohort = self.progressive_key_cohort(output.select.group.len())?;
        let mut groups = Vec::new();
        if output.select.group.is_empty() {
            let group = self.having_group(Vec::new(), &expressions)?;
            self.push(&mut groups, group)?;
        }
        Ok(HavingInput {
            expressions,
            count_only,
            lookup,
            policies,
            cohort,
            groups,
        })
    }

    pub(super) fn push_having_input(
        &mut self,
        output: StageOutput<'_, '_>,
        state: &mut HavingInput<'_>,
        row: Vec<CellValue>,
    ) -> Result<(), QueryError> {
        let environment = Environment {
            aggregate_results: None,
            row: &row,
            collections: None,
            outer: &[],
            output: &[],
            group: None,
            group_keys: None,
        };
        if let Some(filter) = &output.select.filter
            && !self.eval_predicate(filter, environment)?.is_true()
        {
            return Ok(());
        }
        let index = if output.select.group.is_empty() {
            0
        } else {
            let mut keys = self.allocate_expression_row(output.select.group.len())?;
            for expression in &output.select.group {
                keys.push(self.repeatable_comparison_key(expression, environment)?);
            }
            if !state.lookup.as_ref().ok_or(QueryError::Source)?.valid(
                &keys,
                self.budget,
                self.options.cancelled,
            )? {
                return Err(QueryError::Source);
            }
            self.observe_progressive_keys(&mut state.cohort, &keys)?;
            let (index, fingerprint) = self.exact_key_match(
                &mut state.lookup,
                &keys,
                &state.groups,
                |group| &group.keys,
                &state.policies,
            )?;
            if let Some(index) = index {
                self.merge_progressive_keys(&mut state.groups[index].keys, &keys)?;
                index
            } else {
                let index = state.groups.len();
                let group = self.having_group(keys, &state.expressions)?;
                self.push(&mut state.groups, group)?;
                state.lookup.as_mut().ok_or(QueryError::Source)?.insert(
                    fingerprint.ok_or(QueryError::Source)?,
                    index,
                    self.budget,
                    self.options.cancelled,
                )?;
                index
            }
        };
        let group = state.groups.get_mut(index).ok_or(QueryError::Source)?;
        for (expression, accumulator) in state.expressions.iter().zip(&mut group.accumulators) {
            self.tick()?;
            let Expr::Aggregate(_, argument) = expression else {
                return Err(QueryError::Source);
            };
            self.push_progressive_aggregate(accumulator, argument.as_deref(), environment)?;
        }
        self.tick()?;
        group.count = group
            .count
            .checked_add(1)
            .ok_or(QueryError::Evaluation(EvalError::Overflow))?;
        if !state.count_only {
            self.retain_having_row(&mut group.rows, row)?;
        }
        Ok(())
    }

    pub(super) fn finish_having_input<E>(
        &mut self,
        output: StageOutput<'_, '_>,
        mut state: HavingInput<'_>,
        emit: &mut RetainedReceiver<'_, Self, E>,
    ) -> Result<(), StreamError<E>> {
        for group in &mut state.groups {
            self.complete_progressive_key_cells(&mut group.keys, &state.cohort)?;
        }
        self.finish_having_cells(
            output,
            HavingPopulation {
                expressions: state.expressions,
                groups: state.groups,
                count_only: state.count_only,
                predicate_count_only: false,
            },
            emit,
        )
    }
}
