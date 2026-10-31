//! Map shared strict TSV lexical errors into the existing write command domain.
use super::super::error::{Error, input};
use crate::commands::tsv;
use tablecraft_core::limits::ResourceBudget;

pub(super) fn validate(
    bytes: &[u8],
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Error> {
    tsv::validate(bytes, budget, cancelled).map_err(|error| match error {
        tsv::Error::Native(error) => Error::Native(error),
        tsv::Error::Malformed(message) => input(message),
    })
}
