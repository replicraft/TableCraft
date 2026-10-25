//! Keys are evaluated once; group equality is deliberately deferred through EOF.
use super::state::{Candidate, Captured, Operations};
use crate::{
    QueryError,
    execution::{
        Environment, Runtime,
        cell::CellValue,
        progressive::{StreamError, StreamSummary, plan::ProgressiveArm, retained::StageOutput},
    },
    external_sources::ExternalBinding,
};

impl Runtime<'_, '_> {
    pub(super) fn capture_text_groups<'p, E>(
        &mut self,
        arm: &mut ProgressiveArm<'p>,
        bindings: &[ExternalBinding],
        summary: &mut StreamSummary,
    ) -> Result<Captured<'p>, StreamError<E>> {
        let source = self.progressive_source(&arm.input, bindings)?;
        let read = arm.read;
        let select = arm.select;
        let output = StageOutput {
            read,
            select,
            aggregates: &arm.aggregates,
        };
        let mut captured =
            self.with_progressive_owner(read.owner, |runtime| runtime.text_group_input(output))?;
        let mut consume = |runtime: &mut Self, row: Vec<CellValue>, _: &mut StreamSummary| {
            runtime
                .push_text_group_input(output, &mut captured, row)
                .map_err(Into::into)
        };
        self.scan_progressive_input(source, &mut arm.input, read.owner, summary, &mut consume)?;
        Ok(captured)
    }

    pub(in crate::execution::progressive) fn text_group_input<'p>(
        &mut self,
        output: StageOutput<'p, '_>,
    ) -> Result<Captured<'p>, QueryError> {
        let select = output.select;
        let having = match &select.having {
            Some(expression) => self.progressive_having_aggregates(expression)?,
            None => Vec::new(),
        };
        let selected_counts_only = self.text_group_counts_only(output.aggregates)?;
        let active_counts_only = if select.having.is_some() {
            self.text_group_counts_only(&having)?
        } else {
            selected_counts_only
        };
        Ok(Captured {
            candidates: Vec::new(),
            cohort: self.progressive_key_cohort(select.group.len())?,
            policies: self.expression_policies(&select.group)?,
            operations: Operations {
                having,
                selected_counts_only,
                active_counts_only,
            },
        })
    }

    pub(in crate::execution::progressive) fn push_text_group_input(
        &mut self,
        output: StageOutput<'_, '_>,
        captured: &mut Captured<'_>,
        row: Vec<CellValue>,
    ) -> Result<(), QueryError> {
        let select = output.select;
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
            && !self.eval_predicate(filter, environment)?.is_true()
        {
            return Ok(());
        }
        let mut keys = self.allocate_expression_row(select.group.len())?;
        for expression in &select.group {
            // Key-state validation and scalar work remain at the written
            // input position; only group membership waits for logical EOF.
            keys.push(self.repeatable_comparison_key(expression, environment)?);
        }
        self.validate_text_group_keys(&keys, &select.group_types)?;
        self.observe_progressive_keys(&mut captured.cohort, &keys)?;
        let retain_rows =
            !captured.operations.selected_counts_only || !captured.operations.active_counts_only;
        let candidate = Candidate {
            keys,
            row: if retain_rows { Some(row) } else { None },
        };
        self.push_text_group_item(&mut captured.candidates, candidate)?;
        Ok(())
    }
}
