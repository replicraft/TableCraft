//! Independent original-population replay over actual captured descendant trees.
use super::population;
use crate::{
    EditError, FixedColumnUpdates, InspectError, NativeImage,
    header::check,
    table::{
        indexes::mutation::normal_index_audit::replacement,
        mutation::{
            fixed_records::indexed::integral_updates::IntegralUpdates, preserved_source::Source,
        },
    },
};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) fn reconstruct(
    image: &NativeImage,
    source: &Source,
    facts: &IntegralUpdates<'_>,
    groups: &[FixedColumnUpdates<'_>],
    expected: &mut [u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    for (position, selected) in facts.checked().indexes().iter().enumerate() {
        check(budget, cancelled)?;
        budget
            .charge(Resource::Steps, position as u64)
            .map_err(InspectError::from)?;
        let selected = selected.index();
        if facts.checked().indexes()[..position]
            .iter()
            .any(|prior| prior.index().data_number() == selected.data_number())
        {
            continue;
        }
        let tree =
            facts
                .ownership
                .require(image, source.table.schema(), selected, budget, cancelled)?;
        let population::Population {
            original,
            final_keys,
        } = population::capture(source, selected, groups, budget, cancelled)?;
        tree.original(&original, budget, cancelled)?;
        replacement::reconstruct(
            image,
            tree,
            &original,
            &final_keys,
            expected,
            budget,
            cancelled,
        )?;
    }
    check(budget, cancelled)?;
    Ok(())
}
