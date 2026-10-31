//! Reconstruct logical results from original source/programs/requests only.
use super::super::{
    BeforeEventGuardError, BeforeEventGuardSelector, BeforeEventIntents, compile, source,
};
use super::evaluate::{self, Execution};
use crate::{InspectError, NativeImage, header::check};
use tablecraft_core::limits::{Resource, ResourceBudget};

pub(super) struct Observed<'a> {
    pub compiled: &'a compile::Compiled,
    pub execution: &'a Execution,
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
    let compiled = observed.compiled;
    let observed = observed.execution;
    let table = source::table(image, table_name, budget, cancelled)?;
    let frames = source::frames(image, &table, original, budget, cancelled)?;
    let rebound = compile::derive_fields(
        image,
        &table,
        selectors,
        original.event(),
        budget,
        cancelled,
    )?;
    let expected = evaluate::run(&rebound, &frames, original.event(), budget, cancelled)?;

    // Source programs can contain large XML strings. Pay comparison work before
    // equality; final row/trace comparisons contain only bounded scalar carriers.
    let mut work = compiled
        .programs
        .len()
        .checked_add(rebound.programs.len())
        .and_then(|count| count.checked_add(expected.trace.len()))
        .and_then(|count| count.checked_add(expected.assignments.len()))
        .ok_or(InspectError::StructureLimit)?;
    for program in compiled.programs.iter().chain(&rebound.programs) {
        work = work
            .checked_add(program.raw.len())
            .and_then(|count| count.checked_add(program.name.len()))
            .ok_or(InspectError::StructureLimit)?;
    }
    for rows in [&expected.rows, &observed.rows].into_iter().flatten() {
        for row in rows {
            check(budget, cancelled)?;
            work = work
                .checked_add(row.values.len())
                .ok_or(InspectError::StructureLimit)?;
        }
    }
    if let Some(rejection) = &expected.rejection {
        work = work
            .checked_add(rejection.description().map_or(0, str::len))
            .ok_or(InspectError::StructureLimit)?;
    }
    if let Some(rejection) = &observed.rejection {
        work = work
            .checked_add(rejection.description().map_or(0, str::len))
            .ok_or(InspectError::StructureLimit)?;
    }
    budget
        .charge(Resource::Steps, work as u64)
        .map_err(InspectError::from)?;
    if rebound.programs != compiled.programs
        || expected.trace != observed.trace
        || expected.assignments != observed.assignments
        || expected.rejection != observed.rejection
        || expected.rows != observed.rows
    {
        return Err(BeforeEventGuardError::Preservation);
    }
    check(budget, cancelled)?;
    Ok(())
}

#[cfg(test)]
mod tests;
