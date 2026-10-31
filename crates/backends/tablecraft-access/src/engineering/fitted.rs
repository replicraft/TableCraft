//! Select one fitted service from original native declarations, without retries.
use tablecraft_ace::{
    Ace16WritePreview, EditError, FittedColumnUpdates, InspectError, NativeImage,
    inspect_table_schema,
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn preview(
    image: NativeImage,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    if let [FittedColumnUpdates::Variable { column, updates }] = groups {
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
        if projected && selected.is_some_and(|field| field.is_ordinary_short_text()) {
            return image
                .preview_ace16_preserved_text_updates(table, column, updates, budget, cancelled);
        }
    }

    image.preview_ace16_fitted_updates(table, groups, budget, cancelled)
}
