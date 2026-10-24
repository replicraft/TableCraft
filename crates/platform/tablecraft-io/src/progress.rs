//! Shared work and cancellation checkpoint for bounded synchronous operations.
use crate::IoError;
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(crate) fn check(
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), IoError> {
    if cancelled() {
        return Err(IoError::Cancelled);
    }

    budget.charge(Resource::Steps, 1)?;
    Ok(())
}
