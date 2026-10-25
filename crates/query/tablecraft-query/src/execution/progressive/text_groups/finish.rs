//! Final rows retain complete original keys instead of an invented input row.
use crate::{
    QueryError,
    execution::{
        Environment, Runtime,
        context::GroupKeys,
        progressive::{
            StreamError,
            having::DeferredGroup,
            retained::{RetainedReceiver, StageOutput},
        },
    },
};

impl Runtime<'_, '_> {
    pub(super) fn finish_text_group_cells<E>(
        &mut self,
        output: StageOutput<'_, '_>,
        groups: Vec<DeferredGroup>,
        emit: &mut RetainedReceiver<'_, Self, E>,
    ) -> Result<(), StreamError<E>> {
        let counts_only = self.text_group_counts_only(output.aggregates)?;
        let mut published = 0_u64;
        for group in groups {
            self.tick()?;
            let results = if counts_only {
                self.progressive_count_results(output.aggregates, group.count)?
            } else {
                self.progressive_aggregate_results(output.aggregates, group.accumulators)?
            };
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
            if output
                .read
                .top
                .is_some_and(|(count, _)| published >= u64::from(count))
            {
                continue;
            }
            emit(self, cells)?;
            published = published.checked_add(1).ok_or(QueryError::StructureLimit)?;
        }
        Ok(())
    }
}
