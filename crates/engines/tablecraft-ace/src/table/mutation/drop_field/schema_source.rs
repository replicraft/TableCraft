//! A DROP-purpose observation layered on actual current native source evidence.
use super::{
    FieldDropError as Error,
    source::{self, Evidence},
};
use crate::{NativeImage, table::mutation::current_schema};
use tablecraft_core::limits::ResourceBudget;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Selection {
    Current,
    Retained,
    RetainedTyped,
    RetainedNonTail,
}

impl Selection {
    fn family(self) -> current_schema::Family {
        match self {
            Self::Current => current_schema::Family::Current,
            Self::Retained | Self::RetainedTyped | Self::RetainedNonTail => {
                current_schema::Family::Retained
            }
        }
    }

    fn evidence(
        self,
        image: &NativeImage,
        table: &str,
        column: &str,
        budget: &mut ResourceBudget,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Evidence, Error> {
        match self {
            Self::Current => {
                Evidence::capture_current_tail(image, table, column, budget, cancelled)
            }
            Self::Retained => {
                Evidence::capture_retained_tail(image, table, column, budget, cancelled)
            }
            Self::RetainedTyped => {
                Evidence::capture_retained_typed_tail(image, table, column, budget, cancelled)
            }
            Self::RetainedNonTail => {
                Evidence::capture_retained_non_tail(image, table, column, budget, cancelled)
            }
        }
    }
}

pub(super) struct Source {
    pub proof: current_schema::Source,
    pub evidence: Evidence,
    pub selection: Selection,
}

pub(super) fn capture(
    image: NativeImage,
    table: &str,
    column: &str,
    selection: Selection,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Source, Error> {
    source::request(table, column, budget, cancelled)?;
    let proof = selection
        .family()
        .capture(image, table, budget, cancelled)
        .map_err(source_error)?;
    let evidence = selection.evidence(proof.image(), table, column, budget, cancelled)?;
    Ok(Source {
        proof,
        evidence,
        selection,
    })
}

pub(super) fn observe(
    image: &NativeImage,
    table: &str,
    column: &str,
    selection: Selection,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Evidence, Error> {
    source::request(table, column, budget, cancelled)?;
    selection
        .family()
        .observe(image, table, budget, cancelled)
        .map_err(source_error)?;
    selection.evidence(image, table, column, budget, cancelled)
}

fn source_error(error: current_schema::Error) -> Error {
    match error {
        current_schema::Error::Inspect(error) => Error::Inspect(error),
        current_schema::Error::Native(error) => Error::Placement(error),
        current_schema::Error::Source => Error::Source,
    }
}
