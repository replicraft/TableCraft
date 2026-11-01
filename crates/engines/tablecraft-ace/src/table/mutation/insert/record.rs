//! Construct a complete ordinary row from declared fields and explicit values.
pub(in crate::table::mutation::insert) mod input;

use crate::table::mutation::record_insertion::{self, fixed as field, layout, payload};
use crate::{EditError, InspectError, TableSchema, header::check};
use tablecraft_core::{limits::ResourceBudget, values::Value};

/// Physical serialization only. The parent owns primary identity, defaults,
/// generated behavior, property validation and operation-specific admission.
pub(super) struct Profile<'a> {
    schema: &'a TableSchema,
    variables: Vec<usize>,
    generated: Option<usize>,
}

impl<'a> Profile<'a> {
    pub(super) fn bind(
        schema: &'a TableSchema,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let variables = layout::variables(schema, budget, cancelled)?;
        Ok(Self {
            schema,
            variables,
            generated: None,
        })
    }

    pub(super) fn bind_incrementing(
        schema: &'a TableSchema,
        column: usize,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let variables = layout::incrementing_variables(schema, column, budget, cancelled)?;
        Ok(Self {
            schema,
            variables,
            generated: Some(column),
        })
    }

    pub(super) fn prepare_input(
        &self,
        values: &[Value],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<input::Input, EditError> {
        if let Some(column) = self.generated {
            input::prepare_incrementing(
                self.schema,
                &self.variables,
                values,
                column,
                budget,
                cancelled,
            )
        } else {
            input::prepare(self.schema, &self.variables, values, budget, cancelled)
        }
    }

    pub(super) fn build(
        &self,
        values: &[Value],
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, EditError> {
        record_insertion::build::build_with(self.schema, &self.variables, values, budget, cancelled)
    }

    pub(super) fn variables(&self) -> &[usize] {
        &self.variables
    }

    pub(super) fn build_prepared(
        &self,
        values: &[Value],
        input: input::Input,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<u8>, EditError> {
        if input.payloads.len() != self.variables.len() {
            return Err(EditError::Preservation);
        }
        budget
            .charge_allocation(input.payloads.len() as u64, size_of::<Vec<u8>>() as u64)
            .map_err(InspectError::from)?;
        let mut payloads = Vec::new();
        payloads
            .try_reserve_exact(input.payloads.len())
            .map_err(|_| InspectError::Allocation)?;
        for payload in input.payloads {
            check(budget, cancelled)?;
            let input::Payload::Bytes(bytes) = payload else {
                return Err(EditError::Preservation);
            };
            payloads.push(bytes);
        }
        record_insertion::build::assemble(
            self.schema,
            &self.variables,
            values,
            payloads,
            budget,
            cancelled,
        )
    }
}

#[cfg(test)]
mod tests;
