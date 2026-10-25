//! Global aggregate occurrences retain independent bounded states through EOF.
use super::{StreamError, StreamSummary, plan::ProgressiveArm};
use crate::{
    OutputType, QueryError,
    execution::{
        Environment, Runtime, aggregate::accumulator::Accumulator, context::AggregateResult,
    },
    plan::{Expr, RelationNode},
    source::ProjectedRow,
};
use tablecraft_core::values::Value;

impl Runtime<'_, '_> {
    pub(super) fn progressive_aggregate_rows<E>(
        &mut self,
        arm: &ProgressiveArm<'_>,
        union_types: Option<&[OutputType]>,
        summary: &mut StreamSummary,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), E>,
    ) -> Result<(), StreamError<E>> {
        self.check_table_schema(&arm.select.source.node)?;
        self.tick()?;
        let RelationNode::Table {
            name,
            columns,
            projection,
            ..
        } = &arm.select.source.node
        else {
            return Err(QueryError::Unsupported.into());
        };
        let mut accumulators = self.progressive_accumulators(&arm.aggregates)?;
        let source = self.source;
        let mut failure: Option<QueryError> = None;
        let mut consume = |row: ProjectedRow<'_>| {
            if let Some(error) = &failure {
                return Err(error.clone());
            }
            let result: Result<(), QueryError> = (|| {
                self.tick()?;
                let row = self.copy_projected_row(columns, projection, row)?;
                summary.scanned_rows = summary
                    .scanned_rows
                    .checked_add(1)
                    .ok_or(QueryError::StructureLimit)?;
                let environment = Environment {
                    aggregate_results: None,
                    row: &row,
                    collections: None,
                    outer: &[],
                    output: &[],
                    group: None,
                    group_keys: None,
                };
                if let Some(filter) = &arm.select.filter
                    && !self.eval_predicate(filter, environment)?.is_true()
                {
                    return Ok(());
                }
                // Operands run once per occurrence, in written projection/tree
                // order, after WHERE. Identical aggregates are deliberately separate.
                for (expression, accumulator) in arm.aggregates.iter().zip(&mut accumulators) {
                    self.tick()?;
                    let Expr::Aggregate(_, argument) = expression else {
                        return Err(QueryError::Source);
                    };
                    self.push_progressive_aggregate(accumulator, argument.as_deref(), environment)?;
                }
                Ok(())
            })();
            if let Err(error) = &result {
                failure = Some(error.clone());
            }
            result
        };
        let scanned = source.scan_projected_progressive(name, projection, &mut consume);
        if let Some(error) = failure {
            return Err(StreamError::Query(error));
        }
        scanned?;
        self.tick()?;

        let results = self.progressive_aggregate_results(&arm.aggregates, accumulators)?;

        // Completed results are borrowed only by the final projection. Input
        // callbacks have returned, and no mutable Runtime overlay can leak.
        let environment = Environment {
            aggregate_results: Some(&results),
            row: &[],
            collections: None,
            outer: &[],
            output: &[],
            group: None,
            group_keys: None,
        };
        let cells = self.progressive_projection(arm, environment)?;
        // Admitted TOP counts are positive, so a global aggregate's one row
        // survives TOP. SQL TOP 0 remains a preparation error.
        self.progressive_publish(arm, cells, union_types, summary, visitor)
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
