//! Predicate rejection suppresses every selected aggregate and scalar operation.
use super::{StreamError, state::HavingPopulation};
use crate::execution::progressive::retained::{RetainedReceiver, StageOutput};
use crate::{
    QueryError,
    execution::{
        Environment, Runtime,
        cell::CellValue,
        context::{AggregateResult, GroupKeys},
    },
    plan::{Aggregate, Expr},
};
use tablecraft_core::values::Value;

impl Runtime<'_, '_> {
    pub(in crate::execution::progressive) fn finish_having_cells<E>(
        &mut self,
        output: StageOutput<'_, '_>,
        population: HavingPopulation<'_>,
        emit: &mut RetainedReceiver<'_, Self, E>,
    ) -> Result<(), StreamError<E>> {
        let having = output.select.having.as_ref().ok_or(QueryError::Source)?;
        let mut published = 0_u64;
        for group in population.groups {
            self.tick()?;
            let having_results = if population.predicate_count_only {
                self.progressive_count_results(&population.expressions, group.count)?
            } else {
                self.progressive_aggregate_results(&population.expressions, group.accumulators)?
            };
            let environment = Environment {
                aggregate_results: Some(&having_results),
                row: &[],
                collections: None,
                outer: &[],
                output: &[],
                group: None,
                group_keys: Some(GroupKeys {
                    expressions: &output.select.group,
                    values: &group.keys,
                    retained_values: true,
                }),
            };
            if !self.eval_predicate(having, environment)?.is_true() {
                // Dropping these operation-local rows does not evaluate any
                // selected operands, activate providers or publish raw cells.
                continue;
            }
            let results = if population.count_only {
                self.progressive_count_results(output.aggregates, group.count)?
            } else {
                let mut accumulators = self.progressive_accumulators(output.aggregates)?;
                for row in group.rows {
                    self.tick()?;
                    let input = Environment {
                        aggregate_results: None,
                        row: &row,
                        collections: None,
                        outer: &[],
                        output: &[],
                        group: None,
                        group_keys: None,
                    };
                    for (expression, accumulator) in output.aggregates.iter().zip(&mut accumulators)
                    {
                        self.tick()?;
                        let Expr::Aggregate(_, argument) = expression else {
                            return Err(QueryError::Source.into());
                        };
                        self.push_progressive_aggregate(accumulator, argument.as_deref(), input)?;
                    }
                }
                self.progressive_aggregate_results(output.aggregates, accumulators)?
            };
            let cells = self.progressive_select_projection(
                output.select,
                Environment {
                    aggregate_results: Some(&results),
                    ..environment
                },
            )?;
            if output
                .read
                .top
                .is_some_and(|(count, _)| published >= u64::from(count))
            {
                // TOP does not suppress accepted omitted groups' eager work.
                continue;
            }
            emit(self, cells)?;
            published = published.checked_add(1).ok_or(QueryError::StructureLimit)?;
        }
        Ok(())
    }

    pub(in crate::execution::progressive) fn progressive_count_results<'p>(
        &mut self,
        expressions: &[&'p Expr],
        count: i64,
    ) -> Result<Vec<AggregateResult<'p>>, QueryError> {
        self.budget.charge_allocation(
            expressions.len() as u64,
            size_of::<AggregateResult<'_>>() as u64,
        )?;
        let mut results = Vec::new();
        results
            .try_reserve_exact(expressions.len())
            .map_err(|_| QueryError::Allocation)?;
        for expression in expressions {
            self.tick()?;
            if !matches!(expression, Expr::Aggregate(Aggregate::Count, None)) {
                return Err(QueryError::Source);
            }
            // The signed count was checked once per matched input. Preserve
            // each full syntactic node and the ordinary Count Int64 cell.
            results.push(AggregateResult {
                expression,
                value: CellValue::scalar(Value::Int64(count)),
            });
        }
        Ok(results)
    }
}
