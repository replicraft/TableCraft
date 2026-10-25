//! One admitted arm consumes a retained input pipeline through checked EOF.
use super::{StreamError, StreamSummary, plan::ProgressiveArm};
use crate::{
    OutputType, QueryError,
    execution::{Environment, Runtime, cell::CellValue},
    external_sources::ExternalBinding,
};
use tablecraft_core::values::Value;

impl Runtime<'_, '_> {
    pub(super) fn progressive_rows<E>(
        &mut self,
        arm: &mut ProgressiveArm<'_>,
        union_types: Option<&[OutputType]>,
        bindings: &[ExternalBinding],
        summary: &mut StreamSummary,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), E>,
    ) -> Result<(), StreamError<E>> {
        if !arm.select.group.is_empty() {
            return self.progressive_group_rows(arm, union_types, bindings, summary, visitor);
        }
        if arm.select.aggregate {
            return self.progressive_aggregate_rows(arm, union_types, bindings, summary, visitor);
        }
        let source = self.progressive_source(&arm.input, bindings)?;
        let read = arm.read;
        let select = arm.select;
        let mut published = 0;
        let mut consume = |runtime: &mut Self, row: Vec<CellValue>, summary: &mut StreamSummary| {
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
            let cells = runtime.progressive_select_projection(select, environment)?;
            // TOP caps only this layer's output. Its later input projections,
            // upstream layers and physical EOF validation still execute.
            if read
                .top
                .is_some_and(|(count, _)| published >= u64::from(count))
            {
                return Ok(());
            }
            runtime.progressive_publish_read(read, cells, union_types, summary, visitor)?;
            published = published.checked_add(1).ok_or(QueryError::StructureLimit)?;
            Ok(())
        };
        self.scan_progressive_input(source, &mut arm.input, read.owner, summary, &mut consume)
    }
}
