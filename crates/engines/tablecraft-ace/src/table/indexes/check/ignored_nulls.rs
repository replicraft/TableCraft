//! Count the eligible population before dropping the checked sequential snapshot.
use super::{IndexProfile, TableSnapshot, projection::CheckProjection, tick};
use crate::{IndexConstraintViolation, IndexInspection, InspectError};
use tablecraft_core::limits::{Resource, ResourceBudget};

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
        let Some(constraints) = index
            .constraints()
            .filter(|constraints| constraints.ignore_nulls())
        else {
            counts.push(None);
            continue;
        };
        // This extra profile-building pass resolves bounded key metadata. Pay
        // before construction, separately from the later physical traversal.
        let work = inspection
            .schema()
            .columns()
            .len()
            .checked_mul(index.columns().len())
            .ok_or(InspectError::StructureLimit)?;
        budget.charge(Resource::Steps, work as u64)?;
        match IndexProfile::for_check(index, inspection.schema()) {
            Ok(_) => {}
            Err(InspectError::Unsupported) => {
                counts.push(None);
                continue;
            }
            Err(error) => return Err(error),
        }
        let projection = CheckProjection::new(inspection.schema(), index, budget, cancelled)?;
        let mut count = 0_usize;
        for row in table.rows() {
            tick(budget, cancelled)?;
            let (all_null, any_null) = projection.null_facts(row, budget, cancelled)?;
            if any_null && constraints.required() {
                return Err(IndexConstraintViolation::NullKey.into());
            }
            if !all_null {
                count = count.checked_add(1).ok_or(InspectError::StructureLimit)?;
            }
        }
        counts.push(Some(count));
    }
    Ok(Some(counts))
}
