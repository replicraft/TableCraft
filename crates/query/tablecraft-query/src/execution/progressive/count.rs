//! Global COUNT(*) retains only its checked counter through source EOF.
use super::{StreamError, StreamSummary, plan::ProgressiveArm};
use crate::{
    OutputType, QueryError,
    execution::{Environment, Runtime},
    plan::RelationNode,
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
        let source = self.source;
        let mut count = 0_i64;
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
                count = count.checked_add(1).ok_or(EvalError::Overflow)?;
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

        // Only final global-count projections see this overlay. No source callback
        // runs with it installed, and every Result path restores the prior value.
        let previous = self.progressive_count.replace(count);
        let result = (|| {
            let environment = Environment {
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
        })();
        self.progressive_count = previous;
        result
    }
}
