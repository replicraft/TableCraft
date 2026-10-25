//! Checked EOF precedes every grouped final projection and publication.
use super::{Group, ProgressiveArm, StreamError, StreamSummary};
use crate::{
    OutputType, QueryError,
    execution::{Environment, Runtime, context::GroupKeys},
};
use tablecraft_core::values::Value;

impl Runtime<'_, '_> {
    pub(super) fn finish_progressive_groups<E>(
        &mut self,
        arm: &ProgressiveArm<'_>,
        groups: Vec<Group>,
        union_types: Option<&[OutputType]>,
        summary: &mut StreamSummary,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), E>,
    ) -> Result<(), StreamError<E>> {
        let mut published = 0_u64;
        for group in groups {
            self.tick()?;
            let results =
                self.progressive_aggregate_results(&arm.aggregates, group.accumulators)?;
            let environment = Environment {
                aggregate_results: Some(&results),
                row: &[],
                collections: None,
                outer: &[],
                output: &[],
                group: None,
                group_keys: Some(GroupKeys {
                    expressions: &arm.select.group,
                    values: &group.keys,
                    retained_values: true,
                }),
            };
            let cells = self.progressive_projection(arm, environment)?;
            if arm
                .read
                .top
                .is_some_and(|(count, _)| published >= u64::from(count))
            {
                // As in buffered TOP, omitted groups still finish all states
                // and run eager final expressions before publication is omitted.
                continue;
            }
            self.progressive_publish(arm, cells, union_types, summary, visitor)?;
            published = published.checked_add(1).ok_or(QueryError::StructureLimit)?;
        }
        Ok(())
    }
}
