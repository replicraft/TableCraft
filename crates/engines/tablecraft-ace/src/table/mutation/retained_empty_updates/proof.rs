//! Fresh source/target admission and whole-image identity seal the zero action.
use super::qualify;
use crate::{EditError, FittedColumnUpdates, NativeImage, header::check};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn prove(
    image: &NativeImage,
    table: &str,
    groups: &[FittedColumnUpdates<'_>],
    after: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    qualify(image, table, groups, budget, cancelled)?;
    if !super::super::normal_fitted::binding::equal_bytes(
        image.as_bytes(),
        after,
        budget,
        cancelled,
    )? {
        return Err(EditError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}
