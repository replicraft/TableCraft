//! Sorted TOP is a complete terminal operation over progressive input.
use super::{StreamError, StreamSummary, plan::ProgressiveArm};
use crate::{
    OutputType, QueryError,
    execution::{Environment, Runtime, cell::CellValue, retrieval::OutputRow},
    external_sources::ExternalBinding,
};
use tablecraft_core::{limits::Resource, values::Value};

impl Runtime<'_, '_> {
    pub(super) fn progressive_sorted_top_rows<E>(
        &mut self,
        arm: &mut ProgressiveArm<'_>,
        union_types: Option<&[OutputType]>,
        bindings: &[ExternalBinding],
        summary: &mut StreamSummary,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), E>,
    ) -> Result<(), StreamError<E>> {
        let source = self.progressive_source(&arm.input, bindings)?;
        let read = arm.read;
        let select = arm.select;
        let mut retained = Vec::new();
        let mut consume = |runtime: &mut Self,
                           row: Vec<CellValue>,
                           _: &mut StreamSummary|
         -> Result<(), StreamError<E>> {
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
            // Share written projection/alias and ORDER BY evaluation with the
            // buffered reader. Even a row outside TOP evaluates every key.
            if let Some(projected) = runtime.project(select, read, environment)? {
                runtime.retain_sorted_top_row(&mut retained, projected)?;
            }
            Ok(())
        };
        self.scan_progressive_input(source, &mut arm.input, read.owner, summary, &mut consume)?;

        // No candidate pruning: the complete population supplies Text policies,
        // strict comparison errors, stable order and every boundary tie. EOF
        // and ordering failures therefore precede every publication in this arm.
        let rows =
            self.with_progressive_owner(read.owner, |runtime| runtime.finish_read(retained, read))?;
        for cells in rows {
            self.tick()?;
            self.with_progressive_owner(read.owner, |runtime| {
                runtime.progressive_publish_read(read, cells, union_types, summary, visitor)
            })?;
        }
        Ok(())
    }

    fn retain_sorted_top_row(
        &mut self,
        rows: &mut Vec<OutputRow>,
        row: OutputRow,
    ) -> Result<(), QueryError> {
        self.tick()?;
        if rows.len() == rows.capacity() {
            let additional = rows.len().max(1);
            let capacity = rows
                .len()
                .checked_add(additional)
                .ok_or(QueryError::StructureLimit)?;
            // A reallocation consumes a new envelope and can move every old
            // element. Charge before either allocation or relocation occurs.
            self.budget
                .charge_allocation(capacity as u64, size_of::<OutputRow>() as u64)?;
            let relocation = (rows.len() as u64)
                .checked_mul(size_of::<OutputRow>() as u64)
                .ok_or(QueryError::StructureLimit)?;
            self.budget.charge(Resource::Steps, relocation)?;
            rows.try_reserve_exact(additional)
                .map_err(|_| QueryError::Allocation)?;
        }
        rows.push(row);
        Ok(())
    }
}
