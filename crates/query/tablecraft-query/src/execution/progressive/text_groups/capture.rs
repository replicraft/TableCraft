//! Keys are evaluated once; group equality is deliberately deferred through EOF.
use super::state::{Candidate, Captured, Operations};
use crate::{
    QueryError,
    execution::{
        Environment, Runtime,
        cell::CellValue,
        progressive::{StreamError, StreamSummary, plan::ProgressiveArm},
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
        let mut captured = self.with_progressive_owner(read.owner, |runtime| {
            let having = match &select.having {
                Some(expression) => runtime.progressive_having_aggregates(expression)?,
                None => Vec::new(),
            };
            let selected_counts_only = runtime.text_group_counts_only(&arm.aggregates)?;
            let active_counts_only = if select.having.is_some() {
                runtime.text_group_counts_only(&having)?
            } else {
                selected_counts_only
            };
            Ok::<_, QueryError>(Captured {
                candidates: Vec::new(),
                cohort: runtime.progressive_key_cohort(select.group.len())?,
                policies: runtime.expression_policies(&select.group)?,
                operations: Operations {
                    having,
                    selected_counts_only,
                    active_counts_only,
                },
            })
        })?;
        let retain_rows =
            !captured.operations.selected_counts_only || !captured.operations.active_counts_only;
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
                // This includes ordinary singleton key-state validation and
                // any scalar work explicitly written inside the key expression.
                keys.push(runtime.repeatable_comparison_key(expression, environment)?);
            }
            runtime.validate_text_group_keys(&keys, &select.group_types)?;
            runtime.observe_progressive_keys(&mut captured.cohort, &keys)?;
            let candidate = Candidate {
                keys,
                row: if retain_rows { Some(row) } else { None },
            };
            runtime.push_text_group_item(&mut captured.candidates, candidate)?;
            Ok(())
        };
        self.scan_progressive_input(source, &mut arm.input, read.owner, summary, &mut consume)?;
        Ok(captured)
    }
}
