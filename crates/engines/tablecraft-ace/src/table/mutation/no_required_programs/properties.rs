//! Actual selected LvProp bytes are strict evidence, never an empty selection.
use crate::{
    EditError, InspectError, NativeImage, TableSchema, WriteBlocker, header::check,
    inspect_table_properties, properties::framing, table::mutation::preserved_source,
};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn capture(
    image: &NativeImage,
    schema: &TableSchema,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    let _span = tracing::debug_span!("native.table.no_required_programs.properties").entered();
    check(budget, cancelled)?;
    let properties = inspect_table_properties(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        schema.name(),
        budget,
        cancelled,
    )
    .map_err(property_error)?;
    if properties.object_id() != schema.object_id() || properties.name() != schema.name() {
        return Err(EditError::Preservation);
    }
    if properties.raw_bytes().is_empty() {
        return Err(EditError::Unsupported(WriteBlocker::Properties));
    }
    framing::validate(&properties, budget, cancelled).map_err(property_error)?;
    // Real shared qualification rejects unknown groups/keys/types/flags and
    // NameMap dependencies. PublishToWeb is specifically BYTE/flags0/value0.
    // Source subsequently validates original and final row constraints; UPDATE
    // neither evaluates DefaultValue nor treats rule text as a macro selector.
    preserved_source::properties::qualify(schema, &properties, budget, cancelled)?;
    check(budget, cancelled)?;
    Ok(())
}

fn property_error(error: InspectError) -> EditError {
    match error {
        InspectError::Unsupported => EditError::Unsupported(WriteBlocker::Properties),
        error => error.into(),
    }
}

#[cfg(test)]
mod tests;
