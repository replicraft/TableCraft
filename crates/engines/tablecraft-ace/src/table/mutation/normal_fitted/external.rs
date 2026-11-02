//! Original-bound retained payload edits and complete-page retirement composition.
mod admission;
mod mixed;
pub(super) mod proof;
mod retirement;
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
    if mixed::selected(groups, bound, budget, cancelled)? {
        mixed::stage::capture(image, source, groups, bound, budget, cancelled).map(|_| ())
    } else if retirement::selected(groups, bound, budget, cancelled)? {
        retirement::stage::capture(image, source, groups, bound, false, budget, cancelled)
            .map(|_| ())
    } else {
        stage::capture(image, source, groups, bound, false, budget, cancelled).map(|_| ())
    }
}
