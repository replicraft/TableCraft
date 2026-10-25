//! Final rows retain complete original keys instead of an invented input row.
use crate::{
    OutputType, QueryError,
    execution::{
        Environment, Runtime,
        context::GroupKeys,
        progressive::{StreamError, StreamSummary, having::DeferredGroup, plan::ProgressiveArm},
    },
};
use tablecraft_core::values::Value;

impl Runtime<'_, '_> {
    pub(super) fn finish_progressive_text_groups<E>(
        &mut self,
        arm: &ProgressiveArm<'_>,
        groups: Vec<DeferredGroup>,
        union_types: Option<&[OutputType]>,
        summary: &mut StreamSummary,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), E>,
    ) -> Result<(), StreamError<E>> {
        let counts_only = self.text_group_counts_only(&arm.aggregates)?;
        let mut published = 0_u64;
        for group in groups {
            self.tick()?;
            let results = if counts_only {
                self.progressive_count_results(&arm.aggregates, group.count)?
            } else {
                self.progressive_aggregate_results(&arm.aggregates, group.accumulators)?
            };
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
                continue;
            }
            self.progressive_publish(arm, cells, union_types, summary, visitor)?;
            published = published.checked_add(1).ok_or(QueryError::StructureLimit)?;
        }
        Ok(())
    }
}
