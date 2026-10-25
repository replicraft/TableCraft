//! Scan one arm through EOF without retaining a complete result snapshot.
use super::{StreamError, StreamSummary, plan::ProgressiveArm};
use crate::{
    OutputType, QueryError,
    execution::{Environment, Runtime, cell::into_scalar},
    plan::RelationNode,
    source::ProjectedRow,
};
use tablecraft_core::values::Value;

impl Runtime<'_, '_> {
    pub(super) fn progressive_rows<E>(
        &mut self,
        arm: &ProgressiveArm<'_>,
        union_types: Option<&[OutputType]>,
        summary: &mut StreamSummary,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), E>,
    ) -> Result<(), StreamError<E>> {
        // All schemas were preflighted before the first arm. Check the selected
        // arm again before its exclusive cursor, never from inside a callback.
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
        let mut published = 0;
        let mut failure: Option<QueryError> = None;
        let mut sink_failure = None;
        let mut consume = |row: ProjectedRow<'_>| {
            if let Some(error) = &failure {
                return Err(error.clone());
            }
            let result = (|| {
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
                let mut cells = self.allocate_expression_row(arm.select.projection.len())?;
                for expression in &arm.select.projection {
                    cells.push(self.retained_expression_operand(
                        expression,
                        Environment {
                            output: &cells,
                            ..environment
                        },
                    )?);
                }
                // Each buffered arm applies TOP before the enclosing UNION's
                // conversion and final publication. Omitted cells still incur
                // their original eager expression failures and EOF validation.
                if arm
                    .read
                    .top
                    .is_some_and(|(count, _)| published >= u64::from(count))
                {
                    return Ok(());
                }
                if let Some(types) = union_types {
                    self.promote_union_rows(std::slice::from_mut(&mut cells), types)?;
                }

                let mut values = self.allocate_row(cells.len())?;
                for mut cell in cells {
                    self.tick()?;
                    cell.value = self.evaluator().publish_expression_value(cell.value)?;
                    values.push(into_scalar(cell)?);
                }
                if values.len() != arm.read.names.len() {
                    return Err(QueryError::Width);
                }
                self.tick()?;
                if let Err(error) = visitor(&values) {
                    sink_failure = Some(error);
                    return Err(QueryError::Source);
                }
                published = published.checked_add(1).ok_or(QueryError::StructureLimit)?;
                summary.emitted_rows = summary
                    .emitted_rows
                    .checked_add(1)
                    .ok_or(QueryError::StructureLimit)?;
                Ok(())
            })();
            if let Err(error) = &result {
                failure = Some(error.clone());
            }
            result
        };
        let scanned = source.scan_projected_progressive(name, projection, &mut consume);
        if let Some(error) = sink_failure {
            return Err(StreamError::Sink(error));
        }
        if let Some(error) = failure {
            return Err(StreamError::Query(error));
        }
        scanned?;
        self.tick()?;
        Ok(())
    }
}
