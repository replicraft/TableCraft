//! One exclusive table cursor feeds retained row-local SELECT continuations.
mod plan;

#[cfg(test)]
mod tests;

use super::{StreamError, StreamSummary};
use crate::{
    QueryError, Source,
    execution::{Environment, Runtime, cell::CellValue},
    external_sources::{ExternalBinding, SourceNamespace},
    plan::{ReadPlan, Relation, RelationNode, SelectPlan},
    source::ProjectedRow,
};

pub(super) struct Input<'p> {
    pub(super) leaf: &'p Relation,
    frames: Vec<Frame<'p>>,
}

struct Frame<'p> {
    read: &'p ReadPlan,
    select: &'p SelectPlan,
    delivered: u64,
}

impl<'p> Input<'p> {
    pub(super) fn frame_count(&self) -> usize {
        self.frames.len()
    }

    pub(super) fn reads(&self) -> impl DoubleEndedIterator<Item = &'p ReadPlan> + '_ {
        self.frames.iter().map(|frame| frame.read)
    }
}

impl<'a, 'source> Runtime<'a, 'source> {
    pub(super) fn with_progressive_owner<T, E>(
        &mut self,
        owner: SourceNamespace,
        apply: impl FnOnce(&mut Self) -> Result<T, E>,
    ) -> Result<T, E> {
        let previous = std::mem::replace(&mut self.owner, owner);
        let result = apply(self);
        self.owner = previous;
        result
    }

    pub(super) fn progressive_source(
        &mut self,
        input: &Input<'_>,
        bindings: &[ExternalBinding],
    ) -> Result<&'source dyn Source, QueryError> {
        if !bindings.is_empty() {
            self.register_external_sources(bindings)?;
        }
        self.check_table_schema(&input.leaf.node)?;
        self.tick()?;
        let RelationNode::Table { namespace, .. } = &input.leaf.node else {
            return Err(QueryError::Unsupported);
        };
        if *namespace == SourceNamespace::Root {
            Ok(self.source)
        } else {
            self.namespace_source(*namespace)
        }
    }

    pub(super) fn scan_progressive_input<E>(
        &mut self,
        source: &'source dyn Source,
        input: &mut Input<'_>,
        owner: SourceNamespace,
        summary: &mut StreamSummary,
        consumer: &mut impl FnMut(
            &mut Self,
            Vec<CellValue>,
            &mut StreamSummary,
        ) -> Result<(), StreamError<E>>,
    ) -> Result<(), StreamError<E>> {
        let leaf = input.leaf;
        let mut forward =
            |runtime: &mut Self, mut cells: Vec<CellValue>, summary: &mut StreamSummary| {
                for frame in input.frames.iter_mut().rev() {
                    runtime.tick()?;
                    let next = runtime.with_progressive_owner(frame.read.owner, |runtime| {
                        runtime.progressive_frame(frame, &cells)
                    })?;
                    let Some(next) = next else {
                        return Ok(());
                    };
                    cells = next;
                }
                runtime.with_progressive_owner(owner, |runtime| consumer(runtime, cells, summary))
            };
        self.scan_progressive_leaf(source, leaf, summary, &mut forward)
    }

    pub(super) fn scan_progressive_leaf<E>(
        &mut self,
        source: &'source dyn Source,
        leaf: &Relation,
        summary: &mut StreamSummary,
        consumer: &mut impl FnMut(
            &mut Self,
            Vec<CellValue>,
            &mut StreamSummary,
        ) -> Result<(), StreamError<E>>,
    ) -> Result<(), StreamError<E>> {
        let RelationNode::Table {
            name,
            columns,
            projection,
            ..
        } = &leaf.node
        else {
            return Err(QueryError::Unsupported.into());
        };
        let mut query_failure: Option<QueryError> = None;
        let mut sink_failure = None;
        let mut visit = |row: ProjectedRow<'_>| {
            if sink_failure.is_some() {
                return Err(QueryError::Source);
            }
            if let Some(error) = &query_failure {
                return Err(error.clone());
            }
            let result: Result<(), StreamError<E>> = (|| {
                self.tick()?;
                let cells = self.copy_projected_row(columns, projection, row)?;
                summary.scanned_rows = summary
                    .scanned_rows
                    .checked_add(1)
                    .ok_or(QueryError::StructureLimit)?;
                consumer(self, cells, summary)
            })();
            match result {
                Ok(()) => Ok(()),
                Err(StreamError::Query(error)) => {
                    query_failure = Some(error.clone());
                    Err(error)
                }
                Err(StreamError::Sink(error)) => {
                    sink_failure = Some(error);
                    Err(QueryError::Source)
                }
            }
        };
        let scanned = source.scan_projected_progressive(name, projection, &mut visit);
        if let Some(error) = sink_failure {
            return Err(StreamError::Sink(error));
        }
        if let Some(error) = query_failure {
            return Err(StreamError::Query(error));
        }
        scanned?;
        self.tick()?;
        Ok(())
    }

    fn progressive_frame(
        &mut self,
        frame: &mut Frame<'_>,
        cells: &[CellValue],
    ) -> Result<Option<Vec<CellValue>>, QueryError> {
        self.progressive_row_cells(frame.read, frame.select, &mut frame.delivered, cells)
    }

    pub(super) fn progressive_row_cells(
        &mut self,
        read: &ReadPlan,
        select: &SelectPlan,
        delivered: &mut u64,
        cells: &[CellValue],
    ) -> Result<Option<Vec<CellValue>>, QueryError> {
        let environment = Environment {
            aggregate_results: None,
            row: cells,
            collections: None,
            outer: &[],
            output: &[],
            group: None,
            group_keys: None,
        };
        if let Some(filter) = &select.filter
            && !self.eval_predicate(filter, environment)?.is_true()
        {
            return Ok(None);
        }
        let cells = self.progressive_select_projection(select, environment)?;
        if read
            .top
            .is_some_and(|(count, _)| *delivered >= u64::from(count))
        {
            return Ok(None);
        }
        *delivered = delivered.checked_add(1).ok_or(QueryError::StructureLimit)?;
        Ok(Some(cells))
    }
}
