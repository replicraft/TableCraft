//! Global aggregate occurrences retain independent bounded states through EOF.
use super::{StreamError, StreamSummary, plan::ProgressiveArm};
use crate::{
    OutputType, QueryError,
    execution::{
        Environment, Runtime, aggregate::accumulator::Accumulator, cell::CellValue,
        context::AggregateResult,
    },
    external_sources::ExternalBinding,
    plan::Expr,
};
use tablecraft_core::values::Value;

impl Runtime<'_, '_> {
    pub(super) fn progressive_aggregate_rows<E>(
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
        let mut accumulators = self.with_progressive_owner(read.owner, |runtime| {
            runtime.progressive_accumulators(expressions)
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
            for (expression, accumulator) in expressions.iter().zip(&mut accumulators) {
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
            let results = runtime.progressive_aggregate_results(expressions, accumulators)?;
            let environment = Environment {
                aggregate_results: Some(&results),
                row: &[],
                collections: None,
                outer: &[],
                output: &[],
                group: None,
                group_keys: None,
            };
            let cells = runtime.progressive_select_projection(select, environment)?;
            // Positive TOP retains the one global result; all input layers
            // have already completed their eager work and checked EOF.
            runtime.progressive_publish_read(read, cells, union_types, summary, visitor)
        })
    }

    pub(super) fn progressive_accumulators(
        &mut self,
        expressions: &[&Expr],
    ) -> Result<Vec<Accumulator>, QueryError> {
        self.budget
            .charge_allocation(expressions.len() as u64, size_of::<Accumulator>() as u64)?;
        let mut accumulators = Vec::new();
        accumulators
            .try_reserve_exact(expressions.len())
            .map_err(|_| QueryError::Allocation)?;
        for expression in expressions {
            self.tick()?;
            let Expr::Aggregate(kind, _) = expression else {
                return Err(QueryError::Source);
            };
            // Resolver-dependent String populations remain provisional through
            // EOF; non-String extrema use the same incremental shared state.
            accumulators.push(Accumulator::new(
                *kind,
                self.options.collation_resolver.is_some(),
            )?);
        }
        Ok(accumulators)
    }

    pub(super) fn progressive_aggregate_results<'p>(
        &mut self,
        expressions: &[&'p Expr],
        accumulators: Vec<Accumulator>,
    ) -> Result<Vec<AggregateResult<'p>>, QueryError> {
        self.budget.charge_allocation(
            expressions.len() as u64,
            size_of::<AggregateResult<'_>>() as u64,
        )?;
        let mut results = Vec::new();
        results
            .try_reserve_exact(expressions.len())
            .map_err(|_| QueryError::Allocation)?;
        for (expression, accumulator) in expressions.iter().zip(accumulators) {
            self.tick()?;
            results.push(AggregateResult {
                expression,
                value: self.finish_progressive_aggregate(accumulator, expression)?,
            });
        }

        Ok(results)
    }
}
