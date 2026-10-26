//! Checked EOF precedes every grouped final projection and publication.
use super::Group;
use crate::execution::progressive::{
    StreamError,
    retained::{RetainedReceiver, StageOutput},
};
use crate::{
    QueryError,
    execution::{Environment, Runtime, context::GroupKeys},
};

impl Runtime<'_, '_> {
    pub(super) fn finish_group_cells<E>(
        &mut self,
        output: StageOutput<'_, '_>,
        groups: Vec<Group>,
        emit: &mut RetainedReceiver<'_, Self, E>,
    ) -> Result<(), StreamError<E>> {
        let mut published = 0_u64;
        for group in groups {
            self.tick()?;
            let results =
                self.progressive_aggregate_results(output.aggregates, group.accumulators)?;
            let environment = Environment {
                aggregate_results: Some(&results),
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
            let cells = self.progressive_select_projection(output.select, environment)?;
            if !output.read.distinct
                && output
                    .read
                    .top
                    .is_some_and(|(count, _)| published >= u64::from(count))
            {
                // As in buffered TOP, omitted groups still finish all states
                // and run eager final expressions before publication is omitted.
                continue;
            }
            emit(self, cells)?;
            published = published.checked_add(1).ok_or(QueryError::StructureLimit)?;
        }
        Ok(())
    }
}
