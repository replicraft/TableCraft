//! Replay preserves global input order after every key membership is settled.
use super::state::{Captured, Partition};
use crate::{
    QueryError,
    execution::{Environment, Runtime, progressive::retained::StageOutput},
    plan::Expr,
};

impl Runtime<'_, '_> {
    pub(super) fn replay_text_group_operands(
        &mut self,
        output: StageOutput<'_, '_>,
        captured: &Captured<'_>,
        partition: &mut Partition,
    ) -> Result<(), QueryError> {
        let having = output.select.having.is_some();
        let expressions = if having {
            &captured.operations.having
        } else {
            output.aggregates
        };
        if !captured.operations.active_counts_only {
            for group in &mut partition.groups {
                self.tick()?;
                group.accumulators = self.progressive_accumulators(expressions)?;
            }
        }
        for assigned in std::mem::take(&mut partition.rows) {
            self.tick()?;
            let group = partition
                .groups
                .get_mut(assigned.group)
                .ok_or(QueryError::Source)?;
            if !captured.operations.active_counts_only {
                let environment = Environment {
                    aggregate_results: None,
                    row: &assigned.row,
                    collections: None,
                    outer: &[],
                    output: &[],
                    group: None,
                    group_keys: None,
                };
                for (expression, accumulator) in expressions.iter().zip(&mut group.accumulators) {
                    self.tick()?;
                    let Expr::Aggregate(_, argument) = expression else {
                        return Err(QueryError::Source);
                    };
                    self.push_progressive_aggregate(accumulator, argument.as_deref(), environment)?;
                }
            }
            if having && !captured.operations.selected_counts_only {
                // Selected operands stay untouched until this group's predicate
                // passes. Ownership moves; retained payloads are not cloned.
                self.retain_having_row(&mut group.rows, assigned.row)?;
            }
        }
        Ok(())
    }
}
