//! Preserve generic variable/footer composition, reclaim only the qualified bag.
use super::super::{EditError, record_serialization::record::replace_payload};
use crate::{InspectError, TableSchema, table::record_layout};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn compose(
    schema: &TableSchema,
    column: usize,
    before: &[u8],
    descriptor: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let layout = record_layout::parse(before, schema, budget)?;
    let variable = schema.columns()[column].physical_field().variable;
    let start = *layout
        .offsets
        .get(variable)
        .ok_or(InspectError::Malformed)?;
    let end = *layout
        .offsets
        .get(variable + 1)
        .ok_or(InspectError::Malformed)?;
    // The canonical property is the last nonempty variable payload. No native
    // values may be lost when SDK-compatible shrinking zeroes reclaimed bytes.
    if layout
        .offsets
        .iter()
        .skip(variable + 1)
        .any(|offset| *offset != end)
    {
        return Err(EditError::Preservation);
    }
    let mut after = replace_payload(schema, column, before, descriptor, true, budget, cancelled)?;
    let new_end = start
        .checked_add(descriptor.len())
        .ok_or(InspectError::StructureLimit)?;
    if new_end < end {
        after[new_end..end].fill(0);
    }
    Ok(after)
}
