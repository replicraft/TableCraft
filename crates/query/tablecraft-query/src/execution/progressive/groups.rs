//! Proven integral/NULL keys retain group states rather than input row groups.
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
    plan::{Expr, SelectPlan},
};
use state::Group;
pub(super) use state::{GroupInput, KeyPolicy};
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
        let output = StageOutput {
            read: arm.read,
            select: arm.select,
            aggregates: &arm.aggregates,
        };
        let lookup = arm.group_lookup.take().ok_or(QueryError::Source)?;
        let mut state = self.with_progressive_owner(output.read.owner, |runtime| {
            runtime.group_input(output.select, lookup)
        })?;
        let mut consume = |runtime: &mut Self, row: Vec<CellValue>, _: &mut StreamSummary| {
            runtime
                .push_group_input(output, &mut state, row)
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
            runtime.finish_group_input(output, state, &mut |runtime, cells| {
                runtime.progressive_publish_read(output.read, cells, union_types, summary, visitor)
            })
        })
    }

    pub(super) fn group_input(
        &mut self,
        select: &SelectPlan,
        lookup: ExactKeys,
    ) -> Result<GroupInput, QueryError> {
        Ok(GroupInput {
            lookup: Some(lookup),
            policies: self.expression_policies(&select.group)?,
            cohort: self.progressive_key_cohort(select.group.len())?,
            groups: Vec::new(),
        })
    }

    pub(super) fn push_group_input(
        &mut self,
        output: StageOutput<'_, '_>,
        state: &mut GroupInput,
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
        let index = if let Some(index) = index {
            self.merge_progressive_keys(&mut state.groups[index].keys, &keys)?;
            index
        } else {
            let index = state.groups.len();
            let accumulators = self.progressive_accumulators(output.aggregates)?;
            self.push(&mut state.groups, Group { keys, accumulators })?;
            state.lookup.as_mut().ok_or(QueryError::Source)?.insert(
                fingerprint.ok_or(QueryError::Source)?,
                index,
                self.budget,
                self.options.cancelled,
            )?;
            index
        };
        for (expression, accumulator) in output
            .aggregates
            .iter()
            .zip(&mut state.groups[index].accumulators)
        {
            self.tick()?;
            let Expr::Aggregate(_, argument) = expression else {
                return Err(QueryError::Source);
            };
            self.push_progressive_aggregate(accumulator, argument.as_deref(), environment)?;
        }
        Ok(())
    }

    pub(super) fn finish_group_input<E>(
        &mut self,
        output: StageOutput<'_, '_>,
        mut state: GroupInput,
        emit: &mut RetainedReceiver<'_, Self, E>,
    ) -> Result<(), StreamError<E>> {
        self.complete_progressive_keys(&mut state.groups, &state.cohort)?;
        self.finish_group_cells(output, state.groups, emit)
    }
}
