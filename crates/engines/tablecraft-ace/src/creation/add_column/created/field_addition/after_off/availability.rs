//! Name-width capacity after one retained complete settled-source proof.
use super::super::{FieldAdditionError, FieldAdditionTarget, build};
use super::{
    FieldAdditionAfterOffError as Error, FieldAdditionAfterOffHistoryError, history,
    source::{ProvedImage, Source},
};
use crate::{
    CreateColumn, InspectError, NativeImage,
    creation::{
        input::{Columns, Rows},
        profile, properties,
    },
    header::check,
    table::mutation::catalog_property,
};
use tablecraft_core::limits::ResourceBudget;

const DESCRIPTOR_BYTES: usize = 3 * size_of::<u32>();

pub(in crate::creation::add_column::created::field_addition) fn inspect(
    image: NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Option<u8>, Option<u8>), Error> {
    let source = Source::capture(image, table, budget, cancelled)?;
    limits(source, budget, cancelled)
}

/// Reuse the actual full-compared Replay; no named or target AFTER is built.
pub(in crate::creation::add_column::created::field_addition) fn inspect_history(
    image: NativeImage,
    table: &str,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Option<u8>, Option<u8>), FieldAdditionAfterOffHistoryError> {
    let proof = history::qualified_replay(&image, table, budget, cancelled)?;
    let source = Source::from_proof(proof, table, budget, cancelled)?;
    Ok(limits(source, budget, cancelled)?)
}

fn limits<P: ProvedImage>(
    source: Source<P>,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<(Option<u8>, Option<u8>), Error> {
    let mut columns = source
        .definition
        .borrowed_columns(budget, cancelled)
        .map_err(FieldAdditionError::from)?;
    if columns.len() >= profile::MAX_COLUMNS {
        return Ok((None, None));
    }
    let capacity =
        catalog_property::fitted_property_capacity(source.proof.image(), budget, cancelled)?;
    budget.charge_allocation(1, size_of::<CreateColumn<'_>>() as u64)?;
    columns
        .try_reserve_exact(1)
        .map_err(|_| InspectError::Allocation)?;
    let mut limits = [None, None];
    for (index, target) in [
        FieldAdditionTarget::NumberLongZero,
        FieldAdditionTarget::ShortText {
            max_utf16_units: 255,
            unicode_compression: true,
        },
    ]
    .into_iter()
    .enumerate()
    {
        check(budget, cancelled)?;
        // Zero-width name is a length basis, never a proposed identifier,
        // source hypothesis or image. The real encoder owns all other bytes.
        columns.push(target.declaration("")?);
        let order = build::extend_order(source.order, &columns, target, budget, cancelled)?;
        let base = {
            let prepared = properties::capture(
                Columns::Values(&columns),
                Rows::Values(&[]),
                budget,
                cancelled,
            )
            .map_err(FieldAdditionError::from)?
            .ok_or(Error::Preservation)?
            .with_order(order)
            .map_err(FieldAdditionError::from)?;

            prepared
                .inline_length()
                .checked_sub(DESCRIPTOR_BYTES)
                .ok_or(Error::Preservation)?
        };
        limits[index] = capacity
            .and_then(|available| available.checked_sub(base))
            .map(|remaining| (remaining / size_of::<u16>()).min(super::MAX_ASCII_NAME_BYTES))
            .filter(|units| *units != 0)
            .map(|units| units as u8);
        columns.pop();
    }
    check(budget, cancelled)?;
    Ok((limits[0], limits[1]))
}
