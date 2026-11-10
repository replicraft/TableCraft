//! Record serialization shared by working fitted and placement consumers.
//! Storage encoding and independent proof encoding retain separate owners.
pub(in crate::table::mutation) mod long_value;
pub(in crate::table::mutation) mod policy;
pub(in crate::table::mutation) mod proof;
pub(in crate::table::mutation) mod record;
pub(in crate::table::mutation) mod storage;
pub(in crate::table::mutation) mod variables;

use crate::{EditError, InspectError};
use tablecraft_core::limits::{Resource, ResourceBudget};

fn check(budget: &mut ResourceBudget, cancelled: &dyn Fn() -> bool) -> Result<(), EditError> {
    crate::header::check(budget, cancelled)?;
    Ok(())
}

pub(in crate::table::mutation) fn copy(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(bytes.len() as u64, 1)
        .map_err(InspectError::from)?;
    budget
        .charge(Resource::Steps, bytes.len() as u64)
        .map_err(InspectError::from)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(bytes.len())
        .map_err(|_| InspectError::Allocation)?;
    result.extend_from_slice(bytes);
    Ok(result)
}

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub(in crate::table::mutation) mod historical_fixed;

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub(in crate::table::mutation) mod historical_mixed;

#[cfg(all(feature = "journal", feature = "ace16-create"))]
pub(in crate::table::mutation) mod historical_text;
