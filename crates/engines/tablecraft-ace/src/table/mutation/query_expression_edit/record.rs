//! Encode through native Memo policy and replace through the existing row owner.
use super::super::{
    EditError,
    record_serialization::{
        long_value::{Encoded, StorageChoice},
        record::replace_payload,
        utf16,
    },
};
use crate::{InspectError, TableSchema, header::check, table::record_layout};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn replace(
    schema: &TableSchema,
    before: &[u8],
    replacement: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, EditError> {
    let column = schema
        .columns()
        .iter()
        .position(|field| field.name() == "Expression")
        .ok_or(InspectError::Unsupported)?;
    let layout = record_layout::parse(before, schema, budget)?;
    let variable = schema.columns()[column].field.variable;
    if variable + 2 != layout.offsets.len() {
        return Err(InspectError::Unsupported.into());
    }
    let start = layout.offsets[variable];
    let end = layout.offsets[variable + 1];
    let bytes = utf16::encode(replacement, budget, cancelled)?;
    let encoded = Encoded::Present(bytes);
    if encoded.preferred() != StorageChoice::Inline {
        return Err(InspectError::Unsupported.into());
    }
    let descriptor = encoded.descriptor(StorageChoice::Inline, None, budget, cancelled)?;
    let mut after = replace_payload(schema, column, before, &descriptor, true, budget, cancelled)?;
    let new_end = start
        .checked_add(descriptor.len())
        .ok_or(InspectError::StructureLimit)?;
    if new_end < end {
        // Source qualification requires Expression to be the final variable.
        // This padding is owned by the replaced field, never a trailing value.
        after[new_end..end].fill(0);
    }
    Ok(after)
}
