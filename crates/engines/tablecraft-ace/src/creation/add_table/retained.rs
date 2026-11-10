//! Append construction over live and retired roles, independent of packed history.
mod audit;
mod build;
mod source;

use super::{AddTableError, AddTablePlan, AddTableSourceCapabilities, AddTableSourceProfile};
use crate::{FirstTableDefinition, NativeImage, NativeImageSource, header::check};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn selected(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<bool, AddTableError> {
    source::selected(image, budget, cancelled)
}

pub(super) fn plan(
    image: NativeImage,
    definition: FirstTableDefinition,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<AddTablePlan, AddTableError> {
    let _span = tracing::debug_span!("native.add_table.retained.plan").entered();
    let source = source::capture(&image, &definition, budget, cancelled)?;
    let after = build::build(&image, &definition, &source, budget, cancelled)?;
    let after = NativeImageSource::from_vec(after, budget, cancelled)?;
    audit::verify(&image, &after, &definition, &source, budget, cancelled)?;
    drop(source);
    check(budget, cancelled)?;
    Ok(AddTablePlan {
        before: image,
        after,
        definition,
        profile: AddTableSourceProfile::Retained,
    })
}

pub(super) fn capabilities(
    image: &NativeImage,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<AddTableSourceCapabilities, AddTableError> {
    source::capabilities(image, budget, cancelled)
}
