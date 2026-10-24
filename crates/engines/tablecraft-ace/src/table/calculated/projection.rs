//! Explicit metadata-only stored scalar selection, without expression execution.
use crate::{InspectError, TableFeatures, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};
use tablecraft_schema::Column;

pub(in crate::table) fn projected_columns(
    features: &TableFeatures,
    names: &[&str],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<usize>, Vec<Column>), InspectError> {
    if names.len() > features.schema().columns().len() {
        return Err(InspectError::Malformed);
    }
    budget.charge_allocation(names.len() as u64, size_of::<usize>() as u64)?;
    budget.charge_allocation(names.len() as u64, size_of::<Column>() as u64)?;
    let mut positions = Vec::new();
    positions
        .try_reserve_exact(names.len())
        .map_err(|_| InspectError::Allocation)?;
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(names.len())
        .map_err(|_| InspectError::Allocation)?;
    for (position, field) in features.columns().enumerate() {
        check(budget, cancelled)?;
        let mut selected = false;
        for name in names {
            check(budget, cancelled)?;
            budget.charge(Resource::Steps, 1)?;
            selected |= name.eq_ignore_ascii_case(field.column().name());
        }
        if selected {
            columns.push(field.stored_column(budget)?);
            positions.push(position);
        }
    }
    if positions.len() != names.len() {
        return Err(InspectError::Malformed);
    }
    Ok((positions, columns))
}
