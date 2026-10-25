//! Text membership waits for checked EOF and the complete key-policy cohort.
mod capture;
mod finish;
mod partition;
mod profile;
mod replay;
mod state;

pub(super) use state::Captured;

use super::{
    StreamError, StreamSummary,
    having::HavingPopulation,
    plan::ProgressiveArm,
    retained::{RetainedReceiver, StageOutput},
};
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
        let captured = self.capture_text_groups::<E>(arm, bindings, summary)?;
        let output = StageOutput {
            read: arm.read,
            select: arm.select,
            aggregates: &arm.aggregates,
        };
        self.with_progressive_owner(output.read.owner, |runtime| {
            runtime.finish_text_group_input(output, captured, &mut |runtime, cells| {
                runtime.progressive_publish_read(output.read, cells, union_types, summary, visitor)
            })
        })
    }

    pub(super) fn finish_text_group_input<E>(
        &mut self,
        output: StageOutput<'_, '_>,
        mut captured: Captured<'_>,
        emit: &mut RetainedReceiver<'_, Self, E>,
    ) -> Result<(), StreamError<E>> {
        let mut partition = self.partition_text_groups(&mut captured)?;
        self.replay_text_group_operands(output, &captured, &mut partition)?;
        if output.select.having.is_some() {
            self.finish_having_cells(
                output,
                HavingPopulation {
                    expressions: captured.operations.having,
                    groups: partition.groups,
                    count_only: captured.operations.selected_counts_only,
                    predicate_count_only: captured.operations.active_counts_only,
                },
                emit,
            )
        } else {
            self.finish_text_group_cells(output, partition.groups, emit)
        }
    }
}

#[cfg(test)]
mod tests;
