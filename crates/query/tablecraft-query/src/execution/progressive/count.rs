//! Global COUNT occurrences retain only checked counters through source EOF.
use super::{StreamError, StreamSummary, plan::ProgressiveArm};
use crate::{
    OutputType, QueryError,
    execution::{Environment, Runtime, context::CountResult},
    plan::{Aggregate, Expr, RelationNode},
    source::ProjectedRow,
};
use tablecraft_core::values::Value;
use tablecraft_expressions::EvalError;

impl Runtime<'_, '_> {
    pub(super) fn progressive_count_rows<E>(
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
        self.budget
            .charge_allocation(arm.counts.len() as u64, size_of::<CountResult<'_>>() as u64)
            .map_err(QueryError::from)?;
        let mut counts = Vec::new();
        counts
            .try_reserve_exact(arm.counts.len())
            .map_err(|_| QueryError::Allocation)?;
        for expression in &arm.counts {
            self.tick()?;
            counts.push(CountResult {
                expression,
                count: 0,
            });
        }
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
                    counts: None,
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
                // order, after WHERE. Identical counts are deliberately separate.
                for result in &mut counts {
                    self.tick()?;
                    let Expr::Aggregate(Aggregate::Count, argument) = result.expression else {
                        return Err(QueryError::Source);
                    };
                    if let Some(argument) = argument
                        && !self.count_argument(argument, environment)?
                    {
                        continue;
                    }
                    result.count = result.count.checked_add(1).ok_or(EvalError::Overflow)?;
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

        // Completed results are borrowed only by the final projection. Input
        // callbacks have returned, and no mutable Runtime overlay can leak.
        let environment = Environment {
            counts: Some(&counts),
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
}
