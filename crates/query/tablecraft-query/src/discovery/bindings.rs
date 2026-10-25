//! Retain pre-schema binding revisions without opening a provider or reading rows.
use super::DiscoveryState;
use crate::{
    QueryError,
    external_sources::{ExternalBinding, MAX_EXTERNAL_SOURCES},
};
use tablecraft_core::limits::{Resource, ResourceBudget};

impl DiscoveryState<'_> {
    pub(crate) fn remember_bindings(
        &self,
        incoming: Vec<ExternalBinding>,
        budget: &mut ResourceBudget,
        cancelled: Option<&dyn Fn() -> bool>,
    ) -> Result<(), QueryError> {
        for binding in incoming {
            check(budget, cancelled)?;
            let mut remembered = self
                .external_sources
                .try_borrow_mut()
                .map_err(|_| QueryError::Source)?;
            let mut known = false;
            for previous in remembered.iter() {
                check(budget, cancelled)?;
                budget.charge(
                    Resource::Steps,
                    previous.comparison_work(binding.request())?,
                )?;
                if previous.matches(binding.request()) {
                    if previous.key != binding.key {
                        return Err(QueryError::SchemaChanged);
                    }
                    known = true;
                    break;
                }
                if previous.key == binding.key {
                    return Err(QueryError::SchemaChanged);
                }
            }
            if !known {
                if remembered.len() == MAX_EXTERNAL_SOURCES {
                    return Err(QueryError::StructureLimit);
                }
                budget.charge_allocation(1, size_of::<ExternalBinding>() as u64)?;
                remembered
                    .try_reserve_exact(1)
                    .map_err(|_| QueryError::Allocation)?;
                remembered.push(binding);
            }
        }
        Ok(())
    }
}

fn check(
    budget: &mut ResourceBudget,
    cancelled: Option<&dyn Fn() -> bool>,
) -> Result<(), QueryError> {
    if cancelled.is_some_and(|cancel| cancel()) {
        return Err(QueryError::Cancelled);
    }
    budget.charge(Resource::Steps, 1)?;
    Ok(())
}
