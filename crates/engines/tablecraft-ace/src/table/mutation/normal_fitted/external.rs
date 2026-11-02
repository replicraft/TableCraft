//! Reuse an existing exclusive external value without allocating or retiring rows.
mod admission;
pub(super) mod proof;
pub(super) mod stage;

use super::binding::Bound;
use crate::table::mutation::preserved_source::Source;
use crate::{EditError, FittedColumnUpdates, NativeImage};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn selected(
    image: &NativeImage,
    source: &Source,
    bound: &Bound,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, EditError> {
    admission::selected(image, source, bound, budget, cancelled)
}

pub(super) fn preflight(
    image: &NativeImage,
    source: &Source,
    groups: &[FittedColumnUpdates<'_>],
    bound: &Bound,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    stage::capture(image, source, groups, bound, budget, cancelled).map(|_| ())
}
