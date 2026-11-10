//! A DROP-purpose observation layered on actual current native source evidence.
use super::super::{
    FieldDropError as Error,
    source::{self, Evidence},
};
use crate::{NativeImage, table::mutation::current_schema};
use tablecraft_core::limits::ResourceBudget;

pub(super) struct Source {
    pub proof: current_schema::Source,
    pub evidence: Evidence,
}

pub(super) fn capture(
    image: NativeImage,
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, Error> {
    source::request(table, column, budget, cancelled)?;
    let proof =
        current_schema::Source::capture(image, table, budget, cancelled).map_err(source_error)?;
    let evidence = Evidence::capture_current_tail(proof.image(), table, column, budget, cancelled)?;
    Ok(Source { proof, evidence })
}

pub(super) fn observe(
    image: &NativeImage,
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Evidence, Error> {
    source::request(table, column, budget, cancelled)?;
    current_schema::observe(image, table, budget, cancelled).map_err(source_error)?;
    Evidence::capture_current_tail(image, table, column, budget, cancelled)
}

pub(super) fn capture_retained(
    image: NativeImage,
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, Error> {
    source::request(table, column, budget, cancelled)?;
    let proof = current_schema::Family::Retained
        .capture(image, table, budget, cancelled)
        .map_err(source_error)?;
    let evidence =
        Evidence::capture_retained_tail(proof.image(), table, column, budget, cancelled)?;
    Ok(Source { proof, evidence })
}

pub(super) fn observe_retained(
    image: &NativeImage,
    table: &str,
    column: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Evidence, Error> {
    source::request(table, column, budget, cancelled)?;
    current_schema::Family::Retained
        .observe(image, table, budget, cancelled)
        .map_err(source_error)?;
    Evidence::capture_retained_tail(image, table, column, budget, cancelled)
}

fn source_error(error: current_schema::Error) -> Error {
    match error {
        current_schema::Error::Inspect(error) => Error::Inspect(error),
        current_schema::Error::Native(error) => Error::Placement(error),
        current_schema::Error::Source => Error::Source,
    }
}
