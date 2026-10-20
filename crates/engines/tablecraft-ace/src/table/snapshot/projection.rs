//! Ordinary scalar selection shared by full snapshots and resumable reads.
use super::*;

pub(in crate::table) fn projected_columns(
    schema: &TableSchema,
    names: &[&str],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Vec<usize>, Vec<Column>), InspectError> {
    if names.len() > schema.columns.len() {
        return Err(InspectError::Malformed);
    }
    budget.charge_allocation(names.len() as u64, size_of::<usize>() as u64)?;
    let mut positions = Vec::new();
    positions
        .try_reserve_exact(names.len())
        .map_err(|_| InspectError::Allocation)?;
    budget.charge_allocation(names.len() as u64, size_of::<Column>() as u64)?;
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(names.len())
        .map_err(|_| InspectError::Allocation)?;
    for (position, column) in schema.columns.iter().enumerate() {
        let mut selected = false;
        for name in names {
            if cancelled() {
                return Err(InspectError::Cancelled);
            }
            budget.charge(Resource::Steps, 1)?;
            selected |= name.eq_ignore_ascii_case(column.name());
        }
        if selected {
            budget.charge_allocation(column.name().len() as u64, 1)?;
            columns.push(column.column()?);
            positions.push(position);
        }
    }
    if positions.len() != names.len() {
        return Err(InspectError::Malformed);
    }
    Ok((positions, columns))
}
