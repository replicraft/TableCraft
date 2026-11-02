//! Preview primary policy composes the shared counter facts and arithmetic.
use super::IncrementingAutoNumber;
use crate::{
    EditError, NativeImage, TableCheck,
    table::mutation::{
        autonumber_counter::{self, Plan, Source},
        insert::{counters, primary_column},
    },
};
use tablecraft_core::limits::ResourceBudget;

impl Plan {
    pub(in crate::table::mutation::insert) fn patch(&self) -> Option<counters::Patch> {
        (self.source.before != self.after).then_some(counters::Patch {
            offset: self.source.offset,
            before: self.source.before,
            after: self.after,
        })
    }
}

pub(in crate::table::mutation::insert) fn bind(
    image: &NativeImage,
    checked: &TableCheck,
    policy: IncrementingAutoNumber,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    let source = source(image, checked, budget, cancelled)?;
    if source.last != policy.expected_last() {
        return Err(EditError::Conflict);
    }
    Ok(source)
}

pub(in crate::table::mutation::insert) fn source(
    image: &NativeImage,
    checked: &TableCheck,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, EditError> {
    let column = primary_column::resolve(checked, budget, cancelled)?;
    autonumber_counter::capture(image, checked.schema(), column, budget, cancelled)
}
