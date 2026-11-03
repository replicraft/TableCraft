//! Paid complete definition binding shared by actual indexed UPDATE tokens.
use crate::{EditError, InspectError, TableSchema, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn require_definition(
    expected: &TableSchema,
    observed: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    // same_definition compares both physical fields and native column vectors.
    // Pay each represented name and complete declaration envelope beforehand.
    let mut work = 0_u64;
    for schema in [expected, observed] {
        check(budget, cancelled)?;
        work = work
            .checked_add(size_of_val(schema.columns()) as u64)
            .and_then(|n| n.checked_add(size_of_val(schema.layout.fields.as_slice()) as u64))
            .and_then(|n| n.checked_add(schema.name().len() as u64))
            .ok_or(InspectError::StructureLimit)?;
        for field in &schema.layout.fields {
            check(budget, cancelled)?;
            work = work
                .checked_add(field.name.len() as u64)
                .ok_or(InspectError::StructureLimit)?;
        }
        for field in schema.columns() {
            check(budget, cancelled)?;
            work = work
                .checked_add(field.name().len() as u64)
                .and_then(|n| n.checked_add(field.field.name.len() as u64))
                .ok_or(InspectError::StructureLimit)?;
        }
    }
    budget
        .charge(Resource::Steps, work)
        .map_err(InspectError::from)?;
    if !expected.same_definition(observed) {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}
