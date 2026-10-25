//! Text membership waits for checked EOF and the complete key-policy cohort.
mod capture;
mod finish;
mod partition;
mod profile;
mod replay;
mod state;

use super::{StreamError, StreamSummary, having::HavingPopulation, plan::ProgressiveArm};
use crate::{OutputType, execution::Runtime, external_sources::ExternalBinding};
use tablecraft_core::values::Value;

impl Runtime<'_, '_> {
    pub(super) fn progressive_text_groups<E>(
        &mut self,
        arm: &mut ProgressiveArm<'_>,
        union_types: Option<&[OutputType]>,
        bindings: &[ExternalBinding],
        summary: &mut StreamSummary,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), E>,
    ) -> Result<(), StreamError<E>> {
        let mut captured = self.capture_text_groups::<E>(arm, bindings, summary)?;
        self.with_progressive_owner(arm.read.owner, |runtime| {
            let mut partition = runtime.partition_text_groups(&mut captured)?;
            runtime.replay_text_group_operands(arm, &captured, &mut partition)?;
            if arm.select.having.is_some() {
                runtime.finish_progressive_having(
                    arm,
                    HavingPopulation {
                        expressions: captured.operations.having,
                        groups: partition.groups,
                        count_only: captured.operations.selected_counts_only,
                        predicate_count_only: captured.operations.active_counts_only,
                    },
                    union_types,
                    summary,
                    visitor,
                )
            } else {
                runtime.finish_progressive_text_groups(
                    arm,
                    partition.groups,
                    union_types,
                    summary,
                    visitor,
                )
            }
        })
    }
}

#[cfg(test)]
mod tests;
