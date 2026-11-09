//! Compose through existing declaration, MR2, placement and DATA-page owners.
use super::CreatedDefaultZeroAddColumnError as Error;
use crate::{
    CreateColumn, InspectError,
    creation::{
        input::{Columns, Rows},
        properties,
    },
};
use tablecraft_core::limits::ResourceBudget;

const DESCRIPTOR_BYTES: usize = 3 * size_of::<u32>();

pub(super) fn properties(
    columns: &[CreateColumn<'_>],
    default_first: bool,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<Vec<u8>, Error> {
    let prepared = properties::capture(
        Columns::Values(columns),
        Rows::Values(&[]),
        budget,
        cancelled,
    )?
    .ok_or(Error::Preservation)?;
    let prepared = if default_first {
        prepared.with_default_first()
    } else {
        prepared
    };
    let length = prepared.inline_length();
    budget
        .charge_allocation(length as u64, 1)
        .map_err(InspectError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| InspectError::Allocation)?;
    bytes.resize(length, 0);
    prepared.write_inline(&mut bytes, budget, cancelled)?;
    bytes.drain(..DESCRIPTOR_BYTES);
    Ok(bytes)
}
