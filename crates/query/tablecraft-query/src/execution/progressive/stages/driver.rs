//! Physical EOF starts a sequential frontier of successful stage finalizations.
use super::{InputState, Specification, Stage};
use crate::{
    OutputType, QueryError,
    execution::{
        Runtime,
        cell::CellValue,
        progressive::{
            StreamError, StreamSummary, input::Input, plan::ProgressiveArm,
            retained::RetainedReceiver,
        },
    },
    external_sources::ExternalBinding,
    plan::{ReadNode, SelectPlan},
    profile::MAX_BIND_NODES,
};
use tablecraft_core::values::Value;

impl Runtime<'_, '_> {
    pub(in crate::execution::progressive) fn progressive_stateful_input(
        &mut self,
        input: &Input<'_>,
    ) -> Result<bool, QueryError> {
        for read in input.reads() {
            self.tick()?;
            let ReadNode::Select(select) = &read.node else {
                return Err(QueryError::Unsupported);
            };
            if select.aggregate || select.having.is_some() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn progressive_stages<'p>(
        &mut self,
        arm: &mut ProgressiveArm<'p>,
    ) -> Result<Vec<Stage<'p>>, QueryError> {
        let count = arm
            .input
            .frame_count()
            .checked_add(1)
            .ok_or(QueryError::StructureLimit)?;
        if count > MAX_BIND_NODES {
            return Err(QueryError::StructureLimit);
        }
        self.budget
            .charge_allocation(count as u64, size_of::<Stage<'_>>() as u64)?;
        let mut stages = Vec::new();
        stages
            .try_reserve_exact(count)
            .map_err(|_| QueryError::Allocation)?;
        for read in arm.input.reads().rev() {
            self.tick()?;
            let (select, aggregates) = self.progressive_select(read)?;
            let lookup = self.progressive_group_lookup(select)?;
            let input_width = self.progressive_stage_width(select)?;
            stages.push(self.progressive_stage(
                Specification {
                    read,
                    select,
                    aggregates,
                    input_width,
                },
                lookup,
            )?);
        }
        self.tick()?;
        let input_width = self.progressive_stage_width(arm.select)?;
        let specification = Specification {
            read: arm.read,
            select: arm.select,
            aggregates: std::mem::take(&mut arm.aggregates),
            input_width,
        };
        stages.push(self.progressive_stage(specification, arm.group_lookup.take())?);
        Ok(stages)
    }

    fn progressive_stage_width(&mut self, select: &SelectPlan) -> Result<usize, QueryError> {
        let mut width = 0usize;
        for slot in &select.source.slots {
            self.tick()?;
            if slot.available {
                width = width.checked_add(1).ok_or(QueryError::StructureLimit)?;
            }
        }
        Ok(width)
    }

    pub(in crate::execution::progressive) fn progressive_staged_rows<E>(
        &mut self,
        arm: &mut ProgressiveArm<'_>,
        union_types: Option<&[OutputType]>,
        bindings: &[ExternalBinding],
        summary: &mut StreamSummary,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), E>,
    ) -> Result<(), StreamError<E>> {
        let source = self.progressive_source(&arm.input, bindings)?;
        let mut stages = self.progressive_stages(arm)?;
        let root = arm.read;
        let mut consume = |runtime: &mut Self, row: Vec<CellValue>, summary: &mut StreamSummary| {
            let mut emit = |runtime: &mut Self, cells| {
                runtime.with_progressive_owner(root.owner, |runtime| {
                    runtime.progressive_publish_read(root, cells, union_types, summary, visitor)
                })
            };
            runtime.push_progressive_stages(&mut stages, row, &mut emit)
        };
        self.scan_progressive_leaf(source, arm.input.leaf, summary, &mut consume)?;

        let mut emit = |runtime: &mut Self, cells| {
            runtime.with_progressive_owner(root.owner, |runtime| {
                runtime.progressive_publish_read(root, cells, union_types, summary, visitor)
            })
        };
        for frontier in 0..stages.len() {
            self.tick()?;
            let (upstream, downstream) = stages.split_at_mut(frontier + 1);
            let current = upstream.last_mut().ok_or(QueryError::Source)?;
            let state = current.state.take().ok_or(QueryError::Source)?;
            let output = current.specification.output();
            let mut forward = |runtime: &mut Self, cells| {
                runtime.push_progressive_stages(downstream, cells, &mut emit)
            };
            self.with_progressive_owner(output.read.owner, |runtime| match state {
                InputState::Row(_) => Ok(()),
                InputState::Global(state) => {
                    runtime.finish_global_cells(output, state, &mut forward)
                }
                InputState::Exact(state) => runtime.finish_group_input(output, state, &mut forward),
                InputState::Having(state) => {
                    runtime.finish_having_input(output, state, &mut forward)
                }
                InputState::Text(state) => {
                    runtime.finish_text_group_input(output, state, &mut forward)
                }
                InputState::Finished => Err(QueryError::Source.into()),
            })?;
            // Taking the state above is only an in-progress finish. Downstream
            // finalization is legal after all upstream work succeeds, including
            // projections omitted by that upstream stage's TOP.
            current.state = Some(InputState::Finished);
        }
        Ok(())
    }

    fn push_progressive_stages<E>(
        &mut self,
        stages: &mut [Stage<'_>],
        mut row: Vec<CellValue>,
        emit: &mut RetainedReceiver<'_, Self, E>,
    ) -> Result<(), StreamError<E>> {
        for stage in stages {
            self.tick()?;
            if row.len() != stage.specification.input_width {
                return Err(QueryError::Width.into());
            }
            let output = stage.specification.output();
            let state = stage.state.as_mut().ok_or(QueryError::Source)?;
            let next = self.with_progressive_owner(output.read.owner, |runtime| match state {
                InputState::Row(delivered) => {
                    runtime.progressive_row_cells(output.read, output.select, delivered, &row)
                }
                InputState::Global(state) => {
                    runtime.push_global_input(output, state, row)?;
                    Ok(None)
                }
                InputState::Exact(state) => {
                    runtime.push_group_input(output, state, row)?;
                    Ok(None)
                }
                InputState::Having(state) => {
                    runtime.push_having_input(output, state, row)?;
                    Ok(None)
                }
                InputState::Text(state) => {
                    runtime.push_text_group_input(output, state, row)?;
                    Ok(None)
                }
                InputState::Finished => Err(QueryError::Source),
            })?;
            let Some(next) = next else {
                return Ok(());
            };
            row = next;
        }
        emit(self, row)
    }
}
