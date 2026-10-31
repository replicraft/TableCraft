//! Select one fitting service without changing the explicit DATA-growth profile.
use super::variable;
use tablecraft_ace::{Ace16WritePreview, EditError, FittedColumnUpdates, NativeImage};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn preview(
    image: NativeImage,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Ace16WritePreview, EditError> {
    if let [FittedColumnUpdates::Variable { column, updates }] = groups
        && variable::preserved_short_text(&image, table, column, budget, cancelled)?
    {
        return image
            .preview_ace16_preserved_text_updates(table, column, updates, budget, cancelled);
    }
    image.preview_ace16_fitted_updates(table, groups, budget, cancelled)
}
