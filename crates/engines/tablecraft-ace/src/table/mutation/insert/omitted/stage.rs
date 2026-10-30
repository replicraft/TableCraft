//! Normalize defaults once, then consume the real existing INSERT builders.
use super::{Original, Prepared, prepare};
use crate::table::mutation::insert::{
    AutoNumberRowInsert, RowInsert, Staged, incrementing, prepare_with, primary_column, record,
};
use crate::{EditError, InspectError, NativeImage, TableCheck, header::check};
use tablecraft_core::limits::ResourceBudget;

pub(in crate::table::mutation::insert) fn build(
    image: &NativeImage,
    checked: &TableCheck,
    original: Original<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged, EditError> {
    let prepared = prepare(image, checked, original, budget, cancelled)?;
    consume(image, checked, prepared, budget, cancelled)
}

pub(in crate::table::mutation::insert) fn consume(
    image: &NativeImage,
    checked: &TableCheck,
    prepared: Prepared<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Staged, EditError> {
    check(budget, cancelled)?;
    match prepared.original {
        Original::Ordinary(_) => {
            let primary = primary_column::resolve(checked, budget, cancelled)?;
            let profile = record::Profile::bind(checked.schema(), budget, cancelled)?;
            budget
                .charge_allocation(prepared.rows.len() as u64, size_of::<RowInsert>() as u64)
                .map_err(InspectError::from)?;
            let mut rows = Vec::new();
            rows.try_reserve_exact(prepared.rows.len())
                .map_err(|_| InspectError::Allocation)?;
            for row in prepared.rows {
                check(budget, cancelled)?;
                rows.push(RowInsert::new(row.values));
            }
            prepare_with(image, checked, &rows, primary, profile, budget, cancelled)
        }
        Original::Incrementing { policy, .. } => {
            budget
                .charge_allocation(
                    prepared.rows.len() as u64,
                    size_of::<AutoNumberRowInsert>() as u64,
                )
                .map_err(InspectError::from)?;
            let mut rows = Vec::new();
            rows.try_reserve_exact(prepared.rows.len())
                .map_err(|_| InspectError::Allocation)?;
            for row in prepared.rows {
                check(budget, cancelled)?;
                rows.push(AutoNumberRowInsert::new(row.values));
            }
            incrementing::prepare(image, checked, &rows, policy, budget, cancelled)
        }
    }
}
