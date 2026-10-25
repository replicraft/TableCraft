//! Shared retained projection, UNION conversion and strict row publication.
use super::{StreamError, StreamSummary, plan::ProgressiveArm};
use crate::{
    OutputType, QueryError,
    execution::{
        Environment, Runtime,
        cell::{CellValue, into_scalar},
    },
};
use tablecraft_core::values::Value;

impl Runtime<'_, '_> {
    pub(super) fn progressive_projection(
        &mut self,
        arm: &ProgressiveArm<'_>,
        environment: Environment<'_>,
    ) -> Result<Vec<CellValue>, QueryError> {
        let mut cells = self.allocate_expression_row(arm.select.projection.len())?;
        for expression in &arm.select.projection {
            cells.push(self.retained_expression_operand(
                expression,
                Environment {
                    output: &cells,
                    ..environment
                },
            )?);
        }
        Ok(cells)
    }

    pub(super) fn progressive_publish<E>(
        &mut self,
        arm: &ProgressiveArm<'_>,
        mut cells: Vec<CellValue>,
        union_types: Option<&[OutputType]>,
        summary: &mut StreamSummary,
        visitor: &mut dyn FnMut(&[Value]) -> Result<(), E>,
    ) -> Result<(), StreamError<E>> {
        if let Some(types) = union_types {
            self.promote_union_rows(std::slice::from_mut(&mut cells), types)?;
        }
        let mut values = self.allocate_row(cells.len())?;
        for mut cell in cells {
            self.tick()?;
            cell.value = self
                .evaluator()
                .publish_expression_value(cell.value)
                .map_err(QueryError::Evaluation)?;
            values.push(into_scalar(cell).map_err(QueryError::Evaluation)?);
        }
        if values.len() != arm.read.names.len() {
            return Err(QueryError::Width.into());
        }
        self.tick()?;
        visitor(&values).map_err(StreamError::Sink)?;
        summary.emitted_rows = summary
            .emitted_rows
            .checked_add(1)
            .ok_or(QueryError::StructureLimit)?;
        Ok(())
    }
}
