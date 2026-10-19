use super::super::ConstraintViolation;
use crate::{InspectError, format::layout::INDEX_KEY_COLUMNS, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn validate_keys<K: Ord>(
    mut parents: Vec<K>,
    mut children: Vec<K>,
    one_to_one: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    sort(&mut parents, budget, cancelled)?;
    for pair in parents.windows(2) {
        check(budget, cancelled)?;
        if pair[0] == pair[1] {
            return Err(ConstraintViolation::DuplicateParent.into());
        }
    }
    if one_to_one {
        sort(&mut children, budget, cancelled)?;
        for pair in children.windows(2) {
            check(budget, cancelled)?;
            if pair[0] == pair[1] {
                return Err(ConstraintViolation::DuplicateChild.into());
            }
        }
    }
    let search_steps =
        (u64::from(parents.len().checked_ilog2().unwrap_or(0)) + 1) * INDEX_KEY_COLUMNS as u64;
    for key in children {
        check(budget, cancelled)?;
        budget.charge(Resource::Steps, search_steps)?;
        if parents.binary_search(&key).is_err() {
            return Err(ConstraintViolation::MissingParent.into());
        }
    }
    Ok(())
}

fn sort<K: Ord>(
    keys: &mut [K],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), InspectError> {
    check(budget, cancelled)?;
    let levels = u64::from(keys.len().checked_ilog2().unwrap_or(0)) + 1;
    budget.charge(
        Resource::Steps,
        keys.len() as u64 * levels * INDEX_KEY_COLUMNS as u64,
    )?;
    keys.sort_unstable();
    check(budget, cancelled)
}
