//! Global aggregate occurrences retain independent bounded states through EOF.
use super::{
    StreamError, StreamSummary,
    plan::ProgressiveArm,
    retained::{RetainedReceiver, StageOutput},
};
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

pub(super) struct GlobalInput {
    accumulators: Vec<Accumulator>,
}

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
        let output = StageOutput {
            read: arm.read,
            select: arm.select,
            aggregates: &arm.aggregates,
        };
        let mut state =
            self.with_progressive_owner(output.read.owner, |runtime| runtime.global_input(output))?;
        let mut consume = |runtime: &mut Self, row: Vec<CellValue>, _: &mut StreamSummary| {
            runtime
                .push_global_input(output, &mut state, row)
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
            runtime.finish_global_cells(output, state, &mut |runtime, cells| {
                runtime.progressive_publish_read(output.read, cells, union_types, summary, visitor)
            })
        })
    }

    pub(super) fn global_input(
        &mut self,
        output: StageOutput<'_, '_>,
    ) -> Result<GlobalInput, QueryError> {
        Ok(GlobalInput {
            accumulators: self.progressive_accumulators(output.aggregates)?,
        })
    }

    pub(super) fn push_global_input(
        &mut self,
        output: StageOutput<'_, '_>,
        state: &mut GlobalInput,
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
        for (expression, accumulator) in output.aggregates.iter().zip(&mut state.accumulators) {
            self.tick()?;
            let Expr::Aggregate(_, argument) = expression else {
                return Err(QueryError::Source);
            };
            self.push_progressive_aggregate(accumulator, argument.as_deref(), environment)?;
        }
        Ok(())
    }

    pub(super) fn finish_global_cells<E>(
        &mut self,
        output: StageOutput<'_, '_>,
        state: GlobalInput,
        emit: &mut RetainedReceiver<'_, Self, E>,
    ) -> Result<(), StreamError<E>> {
        let results = self.progressive_aggregate_results(output.aggregates, state.accumulators)?;
        let environment = Environment {
            aggregate_results: Some(&results),
            row: &[],
            collections: None,
            outer: &[],
            output: &[],
            group: None,
            group_keys: None,
        };
        let cells = self.progressive_select_projection(output.select, environment)?;
        // Positive TOP retains the one global result. The caller supplies
        // logical EOF before invoking finish, even for an empty population.
        emit(self, cells)
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
