//! Paid borrowed ROW-reference lookup preserves the complete written update order.
use crate::table::indexes::mutation::{IndexKeyUpdate, profile::WriteProfile};
use crate::{EditError, InspectError, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) struct Requests<'updates> {
    updates: &'updates [IndexKeyUpdate],
    positions: Vec<usize>,
    null_transition: bool,
}

impl<'updates> Requests<'updates> {
    pub(super) fn new<P: WriteProfile>(
        updates: &'updates [IndexKeyUpdate],
        profile: &P,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Self, EditError> {
        let mut count = 0_usize;
        let mut null_transition = false;
        for update in updates {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, profile.components() as u64)
                .map_err(InspectError::from)?;
            if profile.includes(update.column) {
                count = count.checked_add(1).ok_or(InspectError::StructureLimit)?;
                null_transition |= update.expected.is_none() || update.value.is_none();
            }
        }
        budget
            .charge_allocation(count as u64, size_of::<usize>() as u64)
            .map_err(InspectError::from)?;
        let mut positions = Vec::new();
        positions
            .try_reserve_exact(count)
            .map_err(|_| InspectError::Allocation)?;
        for (position, update) in updates.iter().enumerate() {
            check(budget, cancelled)?;
            budget
                .charge(Resource::Steps, profile.components() as u64)
                .map_err(InspectError::from)?;
            if profile.includes(update.column) {
                positions.push(position);
            }
        }
        let levels = positions.len().checked_ilog2().unwrap_or(0) as usize + 1;
        let sorting = positions
            .len()
            .checked_mul(levels)
            .and_then(|work| work.checked_mul(size_of::<usize>()))
            .ok_or(InspectError::StructureLimit)?;
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, sorting as u64)
            .map_err(InspectError::from)?;
        // The input ordinal is an explicit tie breaker. Do not sort by column,
        // discard duplicate updates or choose just one matching ROW request.
        positions.sort_unstable_by_key(|position| (updates[*position].reference, *position));
        check(budget, cancelled)?;
        Ok(Self {
            updates,
            positions,
            null_transition,
        })
    }

    pub(super) fn row(
        &self,
        reference: u32,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<&[usize], EditError> {
        check(budget, cancelled)?;
        if self.positions.is_empty() {
            return Ok(&[]);
        }
        let levels = self.positions.len().checked_ilog2().unwrap_or(0) as usize + 1;
        let work = levels.checked_mul(2).ok_or(InspectError::StructureLimit)?;
        budget
            .charge(Resource::Steps, work as u64)
            .map_err(InspectError::from)?;
        let start = self
            .positions
            .partition_point(|position| self.updates[*position].reference < reference);
        let end = self
            .positions
            .partition_point(|position| self.updates[*position].reference <= reference);
        check(budget, cancelled)?;
        Ok(&self.positions[start..end])
    }

    pub(super) fn count(&self) -> usize {
        self.positions.len()
    }

    pub(super) fn has_null_transition(&self) -> bool {
        self.null_transition
    }
}

#[cfg(test)]
mod tests;
