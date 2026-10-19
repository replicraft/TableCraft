use super::super::super::{EditError, WriteBlocker};
use crate::{InspectError, NativeImage, TableSnapshot, header::check};
use tablecraft_core::{
    limits::{Resource, ResourceBudget},
    values::Value,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Cell {
    pub table: usize,
    pub row: usize,
    pub column: usize,
}
pub(super) struct Fact {
    pub cell: Cell,
    pub value: Value,
}
pub(super) struct Cells {
    pub tables: Vec<TableSnapshot>,
    pub facts: Vec<Fact>,
}
impl Cells {
    pub fn original(&self, cell: Cell) -> Result<&Value, EditError> {
        self.tables
            .get(cell.table)
            .and_then(|table| table.rows().get(cell.row))
            .and_then(|row| row.get(cell.column))
            .ok_or(EditError::Conflict)
    }
    pub fn value(&self, cell: Cell, budget: &mut ResourceBudget) -> Result<&Value, EditError> {
        budget
            .charge(Resource::Steps, self.facts.len() as u64)
            .map_err(InspectError::from)?;
        match self.facts.iter().find(|fact| fact.cell == cell) {
            Some(fact) => Ok(&fact.value),
            None => self.original(cell),
        }
    }
    // Facts are monotone: one changed target per cell, with identical demands
    // coalesced. Original-valued explicit facts pin a caller's requested no-op.
    pub fn assign(
        &mut self,
        cell: Cell,
        value: Value,
        explicit: bool,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<bool, EditError> {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, self.facts.len() as u64)
            .map_err(InspectError::from)?;
        if let Some(prior) = self.facts.iter().find(|fact| fact.cell == cell) {
            if prior.value != value || explicit {
                return Err(EditError::Conflict);
            }
            return Ok(false);
        }
        if !explicit && self.original(cell)? == &value {
            return Ok(false);
        }
        if self.facts.len() >= NativeImage::MAX_FIXED_BATCH_ASSIGNMENTS {
            return Err(InspectError::StructureLimit.into());
        }
        reserve(&mut self.facts, budget)?;
        self.facts.push(Fact { cell, value });
        Ok(true)
    }
}
pub(super) fn reserve<T>(
    values: &mut Vec<T>,
    budget: &mut ResourceBudget,
) -> Result<(), EditError> {
    budget
        .charge_allocation(1, size_of::<T>() as u64)
        .map_err(InspectError::from)?;
    values
        .try_reserve_exact(1)
        .map_err(|_| InspectError::Allocation)?;
    Ok(())
}
// Clone only fixed-size, allocation-free carriers. Text/extended values
// must never bypass fallible allocation or the fixed writer's type boundary.
pub(super) fn scalar(value: &Value) -> Result<Value, EditError> {
    match value {
        Value::Null
        | Value::Boolean(_)
        | Value::Byte(_)
        | Value::Int16(_)
        | Value::Int32(_)
        | Value::Int64(_)
        | Value::Currency(_)
        | Value::Single(_)
        | Value::Double(_)
        | Value::DateTime(_)
        | Value::Decimal(_)
        | Value::Guid(_) => Ok(value.clone()),
        _ => Err(EditError::Unsupported(WriteBlocker::Column)),
    }
}
