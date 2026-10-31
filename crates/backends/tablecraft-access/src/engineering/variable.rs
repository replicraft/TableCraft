//! Native metadata selects one variable service before any candidate staging.
use tablecraft_ace::{
    Ace16WritePreview, EditError, InspectError, NativeImage, SlotScalarUpdate, inspect_table_schema,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn preserved_short_text(
    image: &NativeImage,
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    let schema = inspect_table_schema(
        &mut image.reader(),
        image.catalog().catalog().scope(),
        table,
        budget,
        cancelled,
    )?;
    let mut projected = false;
    let mut selected = None;
    for field in schema.columns() {
        if cancelled() {
            return Err(InspectError::Cancelled.into());
        }
        budget
            .charge(Resource::Steps, field.name().len().max(column.len()) as u64)
            .map_err(InspectError::from)?;
        projected |= field.scalar_type().is_none();
        if field.name().eq_ignore_ascii_case(column) && selected.replace(field).is_some() {
            return Err(InspectError::Malformed.into());
        }
    }
    Ok(projected && selected.is_some_and(|field| field.is_ordinary_short_text()))
}

pub(super) fn preview(
    image: NativeImage,
    table: &str,
    column: &str,
    rows: &[SlotScalarUpdate],
    index_growth: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    if index_growth {
        return image.preview_ace16_variable_split_updates(table, column, rows, budget, cancelled);
    }
    if preserved_short_text(&image, table, column, budget, cancelled)? {
        return image.preview_ace16_preserved_text_growth(table, column, rows, budget, cancelled);
    }
    image.preview_ace16_variable_updates(table, column, rows, budget, cancelled)
}
