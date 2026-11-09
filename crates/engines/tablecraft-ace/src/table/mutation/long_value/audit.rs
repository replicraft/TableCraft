//! Separate physical framing and semantic descriptor/chunk proofs.
mod chunks;
mod pages;
mod payload;

use super::super::EditError;
use super::{admission::Prepared, stage::Staged};
use crate::NativeImage;
use tablecraft_core::limits::ResourceBudget;

pub(super) fn prove(
    before: &NativeImage,
    after: &NativeImage,
    prepared: &Prepared,
    staged: &Staged<'_>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), EditError> {
    pages::prove(
        before,
        after,
        prepared.structural.schema(),
        staged,
        budget,
        cancelled,
    )?;
    chunks::prove(
        after,
        prepared.structural.schema(),
        prepared.column,
        staged,
        budget,
        cancelled,
    )
}
