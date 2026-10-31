//! Original-property validation of independently reconstructed scalar projections.
use crate::{
    EditError, InspectError, NativeImage, TableSnapshot, header::check, inspect_table_properties,
    read_table_projection,
};
use std::io::Cursor;
use tablecraft_core::limits::ResourceBudget;

pub(super) fn expected(
    image: &NativeImage,
    table: &str,
    original: &TableSnapshot,
    expected: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    check(budget, cancelled)?;
    budget
        .charge_allocation(original.columns().len() as u64, size_of::<&str>() as u64)
        .map_err(InspectError::from)?;
    let mut names = Vec::new();
    names
        .try_reserve_exact(original.columns().len())
        .map_err(|_| InspectError::Allocation)?;
    for column in original.columns() {
        check(budget, cancelled)?;
        names.push(column.name());
    }

    let scope = image.catalog().catalog().scope();
    let projected = read_table_projection(
        &mut Cursor::new(expected),
        scope,
        table,
        &names,
        budget,
        cancelled,
    )?;
    if projected.schema().object_id() != original.schema().object_id()
        || projected.schema().definition_page() != original.schema().definition_page()
        || projected.rows().len() != original.rows().len()
        || projected.columns().len() != original.columns().len()
    {
        return Err(EditError::Preservation);
    }
    let properties =
        inspect_table_properties(&mut image.reader(), scope, table, budget, cancelled)?;
    crate::constraints::check_scalar_projection(&projected, &properties, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(())
}
