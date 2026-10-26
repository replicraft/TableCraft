//! Count the eligible population before dropping the checked sequential snapshot.
use super::{IndexProfile, TableSnapshot, tick};
use crate::{IndexConstraintViolation, IndexInspection, InspectError};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn populations(
    inspection: &IndexInspection,
    table: &TableSnapshot,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Option<Vec<Option<usize>>>, InspectError> {
    let indexes = inspection.indexes();
    let mut present = false;
    for index in indexes {
        tick(budget, cancelled)?;
        present |= index
            .constraints()
            .is_some_and(|constraints| constraints.ignore_nulls());
    }
    if !present {
        return Ok(None);
    }

    budget.charge_allocation(indexes.len() as u64, size_of::<Option<usize>>() as u64)?;
    let mut counts = Vec::new();
    counts
        .try_reserve_exact(indexes.len())
        .map_err(|_| InspectError::Allocation)?;
    for index in indexes {
        tick(budget, cancelled)?;
        let profile = match IndexProfile::ignored_fixed(index, inspection.schema()) {
            Ok(IndexProfile::Single(profile)) => profile,
            Err(InspectError::Unsupported) => {
                counts.push(None);
                continue;
            }
            Ok(_) => return Err(InspectError::Malformed),
            Err(error) => return Err(error),
        };
        let constraints = index.constraints().ok_or(InspectError::Unsupported)?;
        let mut count = 0_usize;
        for row in table.rows() {
            tick(budget, cancelled)?;
            let value = row.get(profile.column).ok_or(InspectError::Malformed)?;
            match profile.key_type.key(value)? {
                None => {
                    // An omitted NULL must not hide a required-key violation.
                    if constraints.required() {
                        return Err(IndexConstraintViolation::NullKey.into());
                    }
                }
                Some(_) => {
                    count = count.checked_add(1).ok_or(InspectError::StructureLimit)?;
                }
            }
        }
        counts.push(Some(count));
    }
    Ok(Some(counts))
}
