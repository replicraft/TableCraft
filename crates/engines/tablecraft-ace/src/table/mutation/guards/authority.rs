//! Rebind immutable original source/intents; staged rows and caches are not inputs.
use super::{
    BeforeEventGuardError, BeforeEventGuardSelector, BeforeEventIntents, BeforeGuardRejection,
    BeforeGuardTrace, compile, evaluate, source,
};
use crate::{NativeImage, header::check};
use tablecraft_core::limits::ResourceBudget;

#[cfg(test)]
mod selector_tests;
#[cfg(test)]
mod tests;

pub(super) struct Observed<'a> {
    pub compiled: &'a compile::Compiled,
    pub trace: &'a [BeforeGuardTrace],
    pub rejection: Option<&'a BeforeGuardRejection>,
}

pub(super) fn prove(
    image: &NativeImage,
    table_name: &str,
    selectors: &[BeforeEventGuardSelector<'_>],
    original: BeforeEventIntents<'_>,
    observed: Observed<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), BeforeEventGuardError> {
    let table = source::table(image, table_name, budget, cancelled)?;
    let frames = source::frames(image, &table, original, budget, cancelled)?;
    let rebound = compile::derive(
        image,
        &table,
        selectors,
        original.event(),
        budget,
        cancelled,
    )?;
    let (expected_trace, expected_rejection) =
        evaluate::run(&rebound, &frames, original.event(), budget, cancelled)?;
    if rebound.programs != observed.compiled.programs
        || expected_trace != observed.trace
        || expected_rejection.as_ref() != observed.rejection
    {
        return Err(BeforeEventGuardError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}
